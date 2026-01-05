use crate::serde_helpers;
use crate::sock::make_simple_sock;
use crate::sock::{ComplexSock, SimpleSock, SockBlockCtl, SocketFactory, SocketParams};
use serde::Deserialize;
use std::cell::RefCell;
use std::io::{Error, ErrorKind, Read, Write};
use std::net::{IpAddr, Shutdown, TcpStream};

/// Configuration for TCP client.
#[derive(Deserialize)]
pub struct TcpClientConfig {
    ip_dst: IpAddr,
    #[serde(
        default = "serde_helpers::default_port",
        deserialize_with = "serde_helpers::string_to_u16"
    )]
    port_dst: u16,
}

type MaybeTcpStream = Option<TcpStream>;

make_simple_sock!(SimpleTcpClient {
    config: TcpClientConfig,
    stream: RefCell<MaybeTcpStream>,
    is_blocking: bool,
}, "tcp-client");

impl SimpleSock for SimpleTcpClient {
    fn open(&mut self) -> std::io::Result<()> {
        self.stream = RefCell::new(Some(TcpStream::connect(format!(
            "{}:{}",
            self.config.ip_dst, self.config.port_dst
        ))?));
        if let Some(stream) = self.stream.borrow().as_ref() {
            return stream.set_nonblocking(!self.is_blocking);
        }
        Ok(())
    }
    fn close(&mut self) {
        self.stream
            .borrow()
            .as_ref()
            .map(|s| s.shutdown(Shutdown::Both));
    }
    fn read(&self, data: &mut [u8], sz: usize) -> std::io::Result<usize> {
        if let Some(stream) = self.stream.borrow_mut().as_mut() {
            match stream.read(data[..sz].as_mut()) {
                Err(e) => {
                    if e.kind() == ErrorKind::WouldBlock {
                        return Ok(0);
                    }
                    return Err(e);
                }
                count => return count,
            }
        }
        Err(Error::from(ErrorKind::NotConnected))
    }
    fn write(&self, data: &[u8], sz: usize) -> std::io::Result<()> {
        if let Some(stream) = self.stream.borrow_mut().as_mut() {
            return stream.write_all(data[..sz].as_ref());
        }
        Err(Error::from(ErrorKind::NotConnected))
    }
}

impl SockBlockCtl for SimpleTcpClient {
    fn set_block(&mut self, is_blocking: bool) -> std::io::Result<()> {
        self.is_blocking = is_blocking;
        Ok(())
    }
}

pub struct TcpClientFactory;

impl TcpClientFactory {
    pub fn new() -> Self {
        Self
    }
}

impl SocketFactory for TcpClientFactory {
    fn create_sock(&self, params: SocketParams) -> std::io::Result<Box<dyn ComplexSock>> {
        // Convert params to JSON value
        let json_value = serde_json::to_value(params)
            .map_err(|_| Error::new(ErrorKind::InvalidInput, "Invalid parameters"))?;

        // Deserialize to TcpClientConfig
        let tcp_config: TcpClientConfig = serde_json::from_value(json_value).map_err(|e| {
            eprintln!("{e}");
            Error::new(ErrorKind::InvalidInput, "Invalid UDP configuration")
        })?;

        // Blocking by default
        Ok(Box::new(SimpleTcpClient::new(
            tcp_config,
            RefCell::new(None),
            true,
        )))
    }
}
