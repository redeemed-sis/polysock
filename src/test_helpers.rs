use crate::sock::{SocketWrapper, SocketFactory};
use std::collections::HashMap;
use std::io;
use std::{time::Duration, fmt::Debug};

pub fn echo_loopback_test<T: Debug + PartialEq>(
    factory: &dyn SocketFactory,
    sender_params: HashMap<String, String>,
    receiver_params: HashMap<String, String>,
    snd_data: Vec<T>,
) -> io::Result<()> {
    let receiver =
        SocketWrapper::new(factory.create_sock_blockctl(receiver_params, false).unwrap());
    let sender =
        SocketWrapper::new(factory.create_sock_blockctl(sender_params, false).unwrap());

    sender.generic_write(snd_data.as_ref(), snd_data.len())?;
    println!("Data sent: {snd_data:?}");
    let mut recv_data: Vec<T> = Vec::new();
    while recv_data.len() < snd_data.len() {
        recv_data.extend(receiver.read_all()?);
        std::thread::sleep(Duration::from_millis(1));
    }
    println!("Data received: {recv_data:?}");
    assert_eq!(recv_data, snd_data);
    Ok(())
}
