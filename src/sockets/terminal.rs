use crate::sock::{ComplexSock, SimpleSock, SockBlockCtl, SocketFactory, SocketParams, make_simple_sock};
use std::io::{self, ErrorKind, Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread::{self, JoinHandle};

fn spawn_stdin_channel() -> (Receiver<Vec<u8>>, JoinHandle<io::Result<()>>, Arc<AtomicBool>) {
    let (tx, rx) = mpsc::channel();
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    let term = SimpleTerminal::default();
    let h = thread::spawn(move || -> io::Result<()>{
        while r.load(Ordering::Relaxed) {
            const CHUNK_SIZE: usize = 1024;
            let mut chunk: Vec<u8> = vec![0; CHUNK_SIZE];
            let sz = read_blocking(&term, &mut chunk, CHUNK_SIZE).unwrap();
            chunk.truncate(sz);
            if tx.send(chunk).is_err() {
                return Err(io::Error::from(ErrorKind::BrokenPipe));
            }
        }
        Ok(())
    });
    (rx, h, running)
}

pub struct SimpleTerminalNonblocking {
    running: Arc<AtomicBool>,
    handle: Option<JoinHandle<io::Result<()>>>,
    stdin: Receiver<Vec<u8>>,
}

type SimpleTermReadCb = fn(obj: &SimpleTerminal, data: &mut [u8], sz: usize) -> io::Result<usize>;

make_simple_sock!(SimpleTerminal {
    non_block_ctl: Option<SimpleTerminalNonblocking>,
    read: SimpleTermReadCb,
}, "stdio");

impl Default for SimpleTerminal {
    fn default() -> Self {
        Self::new(None, read_blocking)
    }
}

fn read_blocking(_: &SimpleTerminal, data: &mut [u8], sz: usize) -> io::Result<usize> {
    {
        let mut stdout = io::stdout().lock();
        print!("stdio# ");
        stdout.flush()?;
    }
    io::stdin().lock().read(data[..sz].as_mut())
}

fn read_nonblocking(obj: &SimpleTerminal, data: &mut [u8], sz: usize) -> io::Result<usize> {
    let ctl = obj.non_block_ctl.as_ref().expect("You can't use nonblocking method without initialization");
    let buf = match ctl.stdin.try_recv() {
        Err(TryRecvError::Empty) => return Ok(0),
        Err(TryRecvError::Disconnected) => return Err(io::Error::from(ErrorKind::ResourceBusy)),
        Ok(buf) => buf,
    };

    let len = if buf.len() < sz {
        buf.len()
    } else {
        sz
    };

    data[..len].copy_from_slice(buf[..len].as_ref());
    Ok(len)
}

impl SimpleSock for SimpleTerminal {
    fn write(&self, data: &[u8], sz: usize) -> io::Result<()> {
        let mut stdout = io::stdout().lock();
        stdout.write_all(data[..sz].as_ref())?;
        stdout.flush()?;
        Ok(())
    }
    fn read(&self, data: &mut [u8], sz: usize) -> io::Result<usize> {
        (self.read)(self, data, sz)
    }
}

impl SockBlockCtl for SimpleTerminal {
    fn set_block(&mut self, is_blocking: bool) -> io::Result<()> {
        if !is_blocking {
            self.read = read_nonblocking;
            let (receiver, handle, running) = spawn_stdin_channel();
            self.non_block_ctl = Some(
                SimpleTerminalNonblocking { running, handle: Some(handle), stdin: receiver }
            );
        } else {
            match &mut self.non_block_ctl {
                None => {},
                Some(ctl) => {
                    ctl.running.store(false, Ordering::Relaxed);
                    // To overcome taking ownership by join() we use Option<>
                    // wrapper
                    let _ = ctl.handle.take().unwrap().join();
                },
            }
            self.read = read_blocking;
        }
        Ok(())
    }
}

impl Drop for SimpleTerminal {
    fn drop(&mut self) {
        if let Some(ctl) = &mut self.non_block_ctl {
            ctl.running.store(false, Ordering::Relaxed);
            let _ = ctl.handle.take().unwrap().join();
        }
    }
}

pub struct SimpleTerminalFactory;

impl SimpleTerminalFactory {
    pub fn new() -> Self {
        Self
    }
}

impl SocketFactory for SimpleTerminalFactory {
    fn create_sock(&self, _: SocketParams) -> io::Result<Box<dyn ComplexSock>> {
        Ok(Box::new(SimpleTerminal::default()))
    }
}

mod tests {
    #![allow(unused_imports)]

    use std::collections::HashMap;

    use crate::{sock::SocketFactory, sockets::terminal::SimpleTerminalFactory, sock::SocketWrapper};

    #[test]
    fn stdout_test() {
        let factory = SimpleTerminalFactory::new();
        let sock = SocketWrapper::new(factory.create_sock(String::new()).unwrap());
        let data: Vec<u8> = sock.read_all().unwrap();
        assert!(sock.generic_write(data.as_ref(), data.len()).is_ok());
    }
}
