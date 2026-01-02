use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::Duration;
use std::{collections::HashMap, io::Result, mem::size_of, thread};
use std::io;

/// A simple socket trait providing basic read/write operations.
#[allow(unused)]
pub trait SimpleSock: Send {
    /// Opens the socket connection.
    fn open(&self) -> Result<()> {
        Ok(())
    }

    /// Closes the socket connection.
    fn close(&self) {}

    /// Reads data into the provided buffer, up to `sz` bytes.
    fn read(&self, data: &mut [u8], sz: usize) -> Result<usize>;

    /// Writes data from the provided buffer, up to `sz` bytes.
    fn write(&self, data: &[u8], sz: usize) -> Result<()>;
}

pub trait SockBlockCtl {
    fn set_block(&mut self, _: bool) -> Result<()> {Ok(())}
}

pub trait SimpleSockBlock: SimpleSock + SockBlockCtl {}

// Any type that impl SimpleSock & SockBlockCtl automatically
// implements SimpleSockBlock
impl<T: SimpleSock + SockBlockCtl> SimpleSockBlock for T {}

pub type SocketParams = HashMap<String, String>;
pub trait SocketFactory {
    /// Creates a new SimpleSock instance with the given parameters.
    fn create_sock(&self, params: SocketParams) -> Result<Box<dyn SimpleSockBlock>>;
    fn create_sock_blockctl(&self, params: SocketParams, is_blocking: bool) -> Result<Box<dyn SimpleSockBlock>> {
        let mut soc = self.create_sock(params)?;
        soc.set_block(is_blocking)?;
        Ok(soc)
    }
}

pub struct SocketManager<'a> {
    in_factory: &'a Box<dyn SocketFactory>,
    out_factory: &'a Box<dyn SocketFactory>,
}

impl <'a> SocketManager<'a> {
    pub fn new(in_factory: &'a Box<dyn SocketFactory>, out_factory: &'a Box<dyn SocketFactory>) -> Self {
        Self { in_factory, out_factory }
    }
    pub fn set_in_factory(&mut self, in_factory: &'a Box<dyn SocketFactory>) {
        self.in_factory = in_factory;
    }
    pub fn set_out_factory(&mut self, out_factory: &'a Box<dyn SocketFactory>) {
        self.out_factory = out_factory;
    }
    pub fn bound_inout(&self, in_params: &SocketParams, out_params: &SocketParams, blocking: bool) -> io::Result<(JoinHandle<Result<()>>, Arc<AtomicBool>)> {
        let input = SocketWrapper::new(self.in_factory.create_sock_blockctl(in_params.clone(), blocking)?);
        let output = SocketWrapper::new(self.out_factory.create_sock(out_params.clone())?);
        let running = Arc::new(AtomicBool::new(true));
        let r = running.clone();

        let h = thread::spawn(move || -> Result<()> {
            while r.load(Ordering::Relaxed) {
                let buf: Vec<u8> = input.read_all()?;
                output.generic_write(buf.as_slice(), buf.len())?;
                // Yeld the thread
                if !blocking {
                    thread::sleep(Duration::from_micros(1));
                }
            }
            Ok(())
        });
        Ok((h, running))
    }
}

pub struct SocketWrapper {
    simple_sock: Box<dyn SimpleSock>,
}

impl SocketWrapper {
    pub fn new(simple_sock: Box<dyn SimpleSock>) -> Self {
        Self {
            simple_sock
        }
    }
    pub fn get_simple_sock(&self) -> &dyn SimpleSock {
        &*self.simple_sock
    }
    /// Reads a vector of generic type T of size `sz`.
    pub fn generic_read<T>(&self, sz: usize) -> Result<Vec<T>> {
        let bytes_needed = size_of::<T>() * sz;
        let mut buffer = vec![0u8; bytes_needed];
        let mut bytes_read = 0;

        while bytes_read < bytes_needed {
            let chunk_iter = bytes_needed - bytes_read;
            let chunk = self.get_simple_sock().read(
                &mut buffer[bytes_read..],
                chunk_iter,
            )?;
            bytes_read += chunk;
            if chunk < chunk_iter {
                break;
            }
        }

        // Convert bytes to Vec<T> safely
        let num_elements = bytes_read / size_of::<T>();
        let mut result = Vec::with_capacity(num_elements);

        for i in 0..num_elements {
            let start = i * size_of::<T>();
            let end = start + size_of::<T>();
            let bytes = &buffer[start..end];

            // Use unsafe only for the necessary conversion
            let value = unsafe { std::ptr::read(bytes.as_ptr() as *const T) };
            result.push(value);
        }

        Ok(result)
    }

    /// Writes a slice of generic type T.
    pub fn generic_write<T>(&self, data: &[T], sz: usize) -> Result<()> {
        let bytes_needed = size_of::<T>() * sz;
        let mut buffer = vec![0u8; bytes_needed];

        // Copy data to buffer safely
        unsafe {
            std::ptr::copy_nonoverlapping(
                data.as_ptr() as *const u8,
                buffer.as_mut_ptr(),
                bytes_needed,
            );
        }

        self.get_simple_sock().write(&buffer, bytes_needed)
    }

    /// Reads all available data of type T in chunks.
    pub fn read_all<T>(&self) -> Result<Vec<T>> {
        const CHUNK_SIZE: usize = 1024; // Reasonable chunk size
        let mut result = Vec::new();

        loop {
            let chunk = self.generic_read::<T>(CHUNK_SIZE)?;
            if chunk.len() < CHUNK_SIZE {
                result.extend(chunk);
                break;
            }
            result.extend(chunk);
        }

        Ok(result)
    }
}
