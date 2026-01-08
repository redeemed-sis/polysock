use std::net::IpAddr;

/// Default local IP address.
pub fn default_ip_local() -> IpAddr {
    "0.0.0.0".parse().unwrap()
}

/// Default local port.
pub fn default_port() -> u16 {
    0
}
