use crate::serde_helpers;
use crate::sock::{SimpleSock, SimpleSockBlock, SockBlockCtl, SocketFactory};
use serde::Deserialize;
use std::collections::HashMap;
use std::io::{self, Error, ErrorKind};
use std::net::{IpAddr, UdpSocket};

/// Default local IP address.
fn default_ip_local() -> IpAddr {
    "0.0.0.0".parse().unwrap()
}

/// Default local port.
fn default_port() -> u16 {
    0
}

/// Configuration for UDP socket.
#[derive(Deserialize)]
struct UdpConfig {
    #[serde(default = "default_ip_local")]
    ip_local: IpAddr,
    ip_dst: Option<IpAddr>,
    #[serde(
        default = "default_port",
        deserialize_with = "serde_helpers::string_to_u16"
    )]
    port_local: u16,
    #[serde(
        default = "default_port",
        deserialize_with = "serde_helpers::string_to_u16"
    )]
    port_dst: u16,
}

/// Simple UDP socket implementation.
struct SimpleUDP {
    _config: UdpConfig, // Prefix with underscore to suppress unused warning
    socket: UdpSocket,
    dst_addr: Option<String>,
}

impl SimpleSock for SimpleUDP {
    fn read(&self, data: &mut [u8], _sz: usize) -> io::Result<usize> {
        // In kind of empty socket we want Ok(0) to return
        match self.socket.recv(data) {
            Err(err) => {
                if err.kind() == ErrorKind::WouldBlock {
                    return Ok(0);
                }
                Err(err)
            }
            count => count,
        }
    }

    fn write(&self, data: &[u8], sz: usize) -> io::Result<()> {
        if sz > 0 {
            if let Some(dst_addr) = &self.dst_addr {
                self.socket.send_to(&data[..sz], dst_addr)?;
            } else {
                return Err(io::Error::from(ErrorKind::InvalidFilename));
            }
        }
        Ok(())
    }
}

impl SockBlockCtl for SimpleUDP {
    fn set_block(&mut self, is_block: bool) -> io::Result<()> {
        // Invert the operation
        self.socket.set_nonblocking(!is_block)
    }
}

/// UDP socket factory implementing the SocketFactory trait.
pub struct SocketFactoryUDP;

impl SocketFactoryUDP {
    /// Creates a new UDP socket factory.
    pub fn new() -> Self {
        Self
    }
}

impl SocketFactory for SocketFactoryUDP {
    fn create_sock(&self, params: HashMap<String, String>) -> io::Result<Box<dyn SimpleSockBlock>> {
        // Convert params to JSON value
        let json_value = serde_json::to_value(params)
            .map_err(|_| Error::new(ErrorKind::InvalidInput, "Invalid parameters"))?;

        // Deserialize to UdpConfig
        let udp_config: UdpConfig = serde_json::from_value(json_value).map_err(|e| {
            eprintln!("{e}");
            Error::new(ErrorKind::InvalidInput, "Invalid UDP configuration")
        })?;

        // Bind and connect the socket
        let socket = UdpSocket::bind(format!("{}:{}", udp_config.ip_local, udp_config.port_local))?;
        let dst_addr = udp_config
            .ip_dst
            .map(|ip_dst| format!("{}:{}", ip_dst, udp_config.port_dst));

        Ok(Box::new(SimpleUDP {
            _config: udp_config,
            socket,
            dst_addr,
        }))
    }
}

mod tests {
    #![allow(unused_imports)]

    use super::*;
    use crate::test_helpers::*;

    #[test]
    fn test_udp_socket_echo_loopback() {
        let factory = SocketFactoryUDP::new();
        let mut sender_params = HashMap::new();
        let mut receiver_params = HashMap::new();
        let port_sender = "8081";
        let port_receiver = "8080";
        let snd_data = "Hello".as_bytes().to_vec();

        sender_params.insert("ip_dst".to_string(), "127.0.0.1".to_string());
        sender_params.insert("port_dst".to_string(), port_receiver.to_string());
        sender_params.insert("port_local".to_string(), port_sender.to_string());

        receiver_params.insert("ip_dst".to_string(), "127.0.0.1".to_string());
        receiver_params.insert("port_dst".to_string(), port_sender.to_string());
        receiver_params.insert("port_local".to_string(), port_receiver.to_string());

        assert!(if let Err(e) =
            echo_loopback_test(&factory, sender_params, receiver_params, snd_data)
        {
            eprintln!("{e}");
            false
        } else {
            true
        })
    }
}
