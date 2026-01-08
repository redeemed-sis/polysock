use crate::serde_helpers;
use crate::sock::{ComplexSock, SimpleSock, SockBlockCtl, SocketFactory, SocketParams, make_simple_sock};
use serde::Deserialize;
use std::io::{self, Error, ErrorKind};
use std::net::{IpAddr, UdpSocket};

/// Configuration for UDP socket.
#[derive(Deserialize)]
pub struct UdpConfig {
    #[serde(default = "serde_helpers::default_ip_local")]
    ip_local: IpAddr,
    ip_dst: Option<IpAddr>,
    #[serde(
        default = "serde_helpers::default_port",
    )]
    port_local: u16,
    #[serde(
        default = "serde_helpers::default_port",
    )]
    port_dst: u16,
}

make_simple_sock!(SimpleUDP {
    _config: UdpConfig,
    socket: UdpSocket,
    dst_addr: Option<String>,
}, "udp");

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
    fn create_sock(&self, params: SocketParams) -> io::Result<Box<dyn ComplexSock>> {
        // Deserialize to UdpConfig
        let udp_config: UdpConfig = serde_json::from_str(params.as_str()).map_err(|e| {
            eprintln!("{e}");
            Error::new(ErrorKind::InvalidInput, "Invalid UDP configuration")
        })?;

        // Bind and connect the socket
        let socket = UdpSocket::bind(format!("{}:{}", udp_config.ip_local, udp_config.port_local))?;
        let dst_addr = udp_config
            .ip_dst
            .map(|ip_dst| format!("{}:{}", ip_dst, udp_config.port_dst));

        Ok(Box::new(SimpleUDP::new(udp_config, socket, dst_addr)))
    }
}

mod tests {
    #![allow(unused_imports)]

    use super::*;
    use crate::test_helpers::*;

    #[test]
    fn test_udp_socket_echo_loopback() {
        let factory = SocketFactoryUDP::new();
        let sender_params =
            "{ \"ip_dst\": \"127.0.0.1\", \"port_dst\": 8080, \"port_local\": 8081}".to_string();
        let receiver_params = 
            "{ \"ip_dst\": \"127.0.0.1\", \"port_dst\": 8081, \"port_local\": 8080}".to_string();
        let snd_data = "Hello".as_bytes().to_vec();

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
