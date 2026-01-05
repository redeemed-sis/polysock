use serde::{Deserialize, Deserializer, de};
use std::result;
use std::str::FromStr;
use std::net::IpAddr;

/// Default local IP address.
pub fn default_ip_local() -> IpAddr {
    "0.0.0.0".parse().unwrap()
}

/// Default local port.
pub fn default_port() -> u16 {
    0
}

pub fn string_to_u16<'de, D>(deserializer: D) -> result::Result<u16, D::Error>
where
    D: Deserializer<'de>,
{
    // 1. Deserialize the value as a string
    let s = String::deserialize(deserializer)?;
    
    // 2. Parse the string into a u16, mapping errors to Serde custom errors
    u16::from_str(&s).map_err(de::Error::custom)
}
