use crate::sock::{SimpleSock, Socket};
use serde::Deserialize;
use std::collections::HashMap;
use std::io::{Error, ErrorKind};
use std::net::{IpAddr, UdpSocket};

fn d_ip_local() -> IpAddr {
    "0.0.0.0".parse().unwrap()
}
fn d_port_local() -> u16 {
    0
}
#[derive(Deserialize)]
struct UdpConfig {
    #[serde(default = "d_ip_local")]
    ip_local: IpAddr,
    ip_dst: IpAddr,
    #[serde(default = "d_port_local")]
    port_local: u16,
    port_dst: u16,
}

struct SimpleUDP {
    conf: UdpConfig,
    sock: UdpSocket,
}

impl SimpleSock for SimpleUDP {
    fn read(&self, data: &mut [u8], _: usize) -> std::io::Result<usize> {
        self.sock.recv(data)
    }
    fn write(&self, data: &[u8], sz: usize) -> std::io::Result<()> {
        self.sock.send(data[..sz].as_ref())?;
        Ok(())
    }
}

pub struct SocketUDP {
    simple_udp: Option<Box<dyn SimpleSock>>,
}

impl SocketUDP {
    pub fn new() -> Self {
        Self { simple_udp: None }
    }
}

impl Socket for SocketUDP {
    fn create_sock(&self, params: HashMap<String, String>) -> std::io::Result<Box<dyn SimpleSock>> {
        let udp_cfg =
            match serde_json::from_value::<UdpConfig>(match serde_json::to_value(params) {
                Ok(val) => val,
                Err(_) => return Err(Error::from(ErrorKind::InvalidInput)),
            }) {
                Err(_) => return Err(Error::from(ErrorKind::InvalidInput)),
                Ok(udp_cfg) => udp_cfg,
            };
        let sock = UdpSocket::bind(format!("{}:{}", udp_cfg.ip_local, udp_cfg.port_local))?;
        sock.connect(format!("{}:{}", udp_cfg.ip_dst, udp_cfg.port_dst))?;
        Ok(Box::new(SimpleUDP {
            conf: udp_cfg,
            sock,
        }))
    }
    fn get_simple_sock(&self) -> &dyn SimpleSock {
        self.simple_udp.as_ref().unwrap().as_ref()
    }
    fn set_simple_sock(&mut self, simple_sock: Box<dyn SimpleSock>) {
        self.simple_udp = Some(simple_sock);
    }
}
