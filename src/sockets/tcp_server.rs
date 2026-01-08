use crate::serde_helpers;
use crate::sock::make_simple_sock;
use crate::sock::{ComplexSock, SimpleSock, SockBlockCtl, SocketFactory, SocketParams};
use pretty_hex::PrettyHex;
use serde::Deserialize;
use std::collections::LinkedList;
use std::io::Write;
use std::io::{self, BufRead, BufReader};
use std::io::{Error, ErrorKind};
use std::net::IpAddr;
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::sync::Mutex;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread::{self, JoinHandle};
use std::time::Duration;

/// Configuration for TCP server.
#[derive(Deserialize)]
pub struct TcpServerConfig {
    #[serde(default = "serde_helpers::default_ip_local")]
    ip_local: IpAddr,
    port_local: u16,
}

type ListenerHandle = JoinHandle<io::Result<()>>;

make_simple_sock!(TcpServer {
    config: TcpServerConfig,
    clients: Arc<Mutex<LinkedList<(TcpStream, SocketAddr)>>>,
    blocking: Arc<AtomicBool>,
    is_running: Arc<AtomicBool>,
    handle: Option<ListenerHandle>,
}, "tcp-server", self, {
    let mut descr = format!("{}{}", self.get_type_name(), self.get_id());
    let clients = self.clients.lock().unwrap();
    if !clients.is_empty() {
        descr.push_str(", connected clients:");
        for (_, addr) in clients.iter() {
            descr.push_str(format!("\nClient {addr}").as_str());
        }
    }
    descr
});

impl SimpleSock for TcpServer {
    fn open(&mut self) -> io::Result<()> {
        let cfg = &self.config;
        let listener = TcpListener::bind(format!("{}:{}", cfg.ip_local, cfg.port_local))?;
        listener.set_nonblocking(true)?;
        self.is_running.store(true, Ordering::Relaxed);
        let r = self.is_running.clone();
        let clients = self.clients.clone();
        let b = self.blocking.clone();

        self.handle = Some(thread::spawn(move || -> io::Result<()> {
            while r.load(Ordering::Relaxed) {
                let cli = if let Ok(cli) = listener.accept() {
                    cli
                } else {
                    // Check acception every 10 ms, it is
                    // bad solution, but it is the easyiest way
                    thread::sleep(Duration::from_millis(10));
                    continue;
                };
                cli.0.set_nonblocking(!b.load(Ordering::Relaxed))?;
                // Pass new connection to client list
                clients.lock().unwrap().push_back(cli);
            }
            Ok(())
        }));
        if self.handle.as_ref().unwrap().is_finished()
            && let Err(e) = self.handle.take().unwrap().join().unwrap()
        {
            return Err(e);
        }
        Ok(())
    }
    fn close(&mut self) {
        self.is_running.store(false, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            // Wait when listener thread is finished
            let _ = handle.join();
            let mut clients = self.clients.lock().unwrap();
            // Invoke shutdown for every connected client
            for (cli, _) in clients.iter() {
                let _ = cli.shutdown(Shutdown::Both);
            }
            // Clear connection list
            clients.clear();
        }
    }
    fn read(&self, data: &mut [u8], sz: usize) -> io::Result<usize> {
        let mut clients = self.clients.lock().unwrap();
        let mut total: usize = 0;

        for (cli, addr) in clients.iter_mut() {
            let mut reader = BufReader::new(cli);
            // Get current internal state of stream
            let tmp = if let Ok(tmp) = reader.fill_buf() {
                tmp
            } else {
                continue;
            };

            let tmp_len = tmp.len();
            // Go to the next client if this empty
            if tmp_len == 0 {
                continue;
            }
            if total + tmp_len > sz || total + tmp_len > data.len() {
                break;
            }
            // Trace data with client address if trace level is trace
            log::trace!("Data received from {}:\n{}", addr, tmp.hex_dump());
            data[total..total + tmp_len].copy_from_slice(tmp);
            total += tmp_len;
            // Now data is really dropped from stream queue
            reader.consume(tmp_len);
        }

        Ok(total)
    }
    fn write(&self, data: &[u8], sz: usize) -> io::Result<()> {
        let mut clients = self.clients.lock().unwrap();

        for (cli, addr) in clients.iter_mut() {
            if cli.write_all(data[..sz].as_ref()).is_ok() {
                log::trace!("Data sent to {}", addr);
            }
        }
        Ok(())
    }
}

impl SockBlockCtl for TcpServer {
    fn set_block(&mut self, is_blocking: bool) -> io::Result<()> {
        self.blocking.store(is_blocking, Ordering::Relaxed);
        Ok(())
    }
}

pub struct TcpServerFactory;

impl TcpServerFactory {
    pub fn new() -> Self {
        Self
    }
}

impl SocketFactory for TcpServerFactory {
    fn create_sock(&self, params: SocketParams) -> io::Result<Box<dyn ComplexSock>> {
        // Deserialize to TcpClientConfig
        let tcp_config: TcpServerConfig = serde_json::from_str(params.as_str()).map_err(|e| {
            eprintln!("{e}");
            Error::new(ErrorKind::InvalidInput, "Invalid UDP configuration")
        })?;

        // Blocking by default
        Ok(Box::new(TcpServer::new(
            tcp_config,
            Arc::new(Mutex::new(LinkedList::new())),
            Arc::new(AtomicBool::new(true)),
            Arc::new(AtomicBool::new(true)),
            None,
        )))
    }
}
