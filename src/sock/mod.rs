pub mod decorators;
pub use decorators::{
    TraceCanonicalDecoratorFactory, TraceInfoDecoratorFactory, TraceRawDecoratorFactory,
};

use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;
use std::{collections::HashMap, io::Result, mem::size_of, thread};

/// A simple socket trait providing basic read/write operations.
#[allow(unused)]
pub trait SimpleSock: Send {
    /// Opens the socket connection.
    fn open(&mut self) -> Result<()> {
        Ok(())
    }

    /// Closes the socket connection.
    fn close(&mut self) {}

    /// Reads data into the provided buffer, up to `sz` bytes.
    fn read(&self, data: &mut [u8], sz: usize) -> Result<usize>;

    /// Writes data from the provided buffer, up to `sz` bytes.
    fn write(&self, data: &[u8], sz: usize) -> Result<()>;
}

pub trait SockInfo {
    fn get_type_name(&self) -> &str;
    fn get_id(&self) -> u32;
    fn get_description(&self) -> String {
        format!("{}{}", self.get_type_name(), self.get_id())
    }
}

pub trait SockBlockCtl {
    fn set_block(&mut self, _: bool) -> Result<()> {
        Ok(())
    }
}

pub trait ComplexSock: SimpleSock + SockBlockCtl + SockInfo {}

// Any type that impl SimpleSock & SockBlockCtl automatically
// implements SimpleSockBlock
impl<T: SimpleSock + SockBlockCtl + SockInfo> ComplexSock for T {}

pub type SocketParams = HashMap<String, String>;
pub trait SocketFactory {
    /// Creates a new SimpleSock instance with the given parameters.
    fn create_sock(&self, params: SocketParams) -> Result<Box<dyn ComplexSock>>;
    fn create_sock_blockctl(
        &self,
        params: SocketParams,
        is_blocking: bool,
    ) -> Result<Box<dyn ComplexSock>> {
        let mut soc = self.create_sock(params)?;
        soc.set_block(is_blocking)?;
        Ok(soc)
    }
}

pub struct SocketManager<'a> {
    in_factory: &'a dyn SocketFactory,
    out_factory: &'a dyn SocketFactory,
}

type DoubleThreadRet = (
    JoinHandle<Result<()>>,
    JoinHandle<Result<()>>,
    Arc<AtomicBool>,
);
type SingleThreadRet = (JoinHandle<Result<()>>, Arc<AtomicBool>);

#[allow(unused)]
impl<'a> SocketManager<'a> {
    pub fn new(in_factory: &'a dyn SocketFactory, out_factory: &'a dyn SocketFactory) -> Self {
        Self {
            in_factory,
            out_factory,
        }
    }
    pub fn set_in_factory(&mut self, in_factory: &'a dyn SocketFactory) {
        self.in_factory = in_factory;
    }
    pub fn set_out_factory(&mut self, out_factory: &'a dyn SocketFactory) {
        self.out_factory = out_factory;
    }
    pub fn bind_unidirectional(
        &self,
        in_params: &SocketParams,
        out_params: &SocketParams,
        blocking: bool,
    ) -> io::Result<SingleThreadRet> {
        let input = SocketWrapper::new(
            self.in_factory
                .create_sock_blockctl(in_params.clone(), blocking)?,
        )
        .open()?;
        let output =
            SocketWrapper::new(self.out_factory.create_sock(out_params.clone())?).open()?;
        let running = Arc::new(AtomicBool::new(true));
        let r = running.clone();

        let h = Self::create_binding_thread(
            Arc::new(Mutex::new(input)),
            Arc::new(Mutex::new(output)),
            r,
        );
        Ok((h, running))
    }
    pub fn bind_bidirectional(
        &self,
        from_params: &SocketParams,
        to_params: &SocketParams,
    ) -> io::Result<DoubleThreadRet> {
        let from = SocketWrapper::new(
            self.in_factory
                .create_sock_blockctl(from_params.clone(), false)?,
        )
        .open()?;
        let to = SocketWrapper::new(
            self.out_factory
                .create_sock_blockctl(to_params.clone(), false)?,
        )
        .open()?;
        let running = Arc::new(AtomicBool::new(true));
        let r_1_2 = running.clone();
        let r_2_1 = running.clone();
        let from_1_2 = Arc::new(Mutex::new(from));
        let to_2_1 = from_1_2.clone();
        let to_1_2 = Arc::new(Mutex::new(to));
        let from_2_1 = to_1_2.clone();

        let handle_1_2 = Self::create_binding_thread(from_1_2, to_1_2, r_1_2);
        let handle_2_1 = Self::create_binding_thread(from_2_1, to_2_1, r_2_1);

        Ok((handle_1_2, handle_2_1, running))
    }
    fn create_binding_thread(
        from: Arc<Mutex<SocketWrapper>>,
        to: Arc<Mutex<SocketWrapper>>,
        r: Arc<AtomicBool>,
    ) -> JoinHandle<Result<()>> {
        thread::spawn(move || -> Result<()> {
            while r.load(Ordering::Relaxed) {
                {
                    let buf: Vec<u8> = from.lock().unwrap().read_all()?;
                    to.lock()
                        .unwrap()
                        .generic_write(buf.as_slice(), buf.len())?;
                }
                // Yeld the thread
                thread::sleep(Duration::from_micros(1));
            }
            Ok(())
        })
    }
}

pub struct SocketWrapper {
    simple_sock: Box<dyn ComplexSock>,
}

impl SocketWrapper {
    pub fn new(simple_sock: Box<dyn ComplexSock>) -> Self {
        Self { simple_sock }
    }
    pub fn open(mut self) -> io::Result<Self> {
        self.simple_sock.open()?;
        Ok(self)
    }
    fn close(&mut self) {
        self.simple_sock.close();
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
            let chunk = self
                .get_simple_sock()
                .read(&mut buffer[bytes_read..], chunk_iter)?;
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

impl Drop for SocketWrapper {
    fn drop(&mut self) {
        self.close();
    }
}

macro_rules! make_simple_sock {
    ($name: ident { $($field:ident : $t:ty),* $(,)? }, $stype: expr $(, $self_ident: ident, $sock_descr: block)?) => {
        paste::paste! {
            use crate::sock::SockInfo;
            use std::sync::atomic::AtomicU32 as IdAtomic;
            use std::sync::atomic::Ordering as IdOrdering;
            #[allow(non_upper_case_globals)]
            static [<$name _id>]: IdAtomic = IdAtomic::new(0);
            pub struct $name {
                stype: String,
                id: u32,
                $($field: $t),*
            }
            impl $name {
                pub fn new($($field: $t),*) -> Self {
                    Self {
                        id: [<$name _id>].fetch_add(1, IdOrdering::Relaxed),
                        stype: $stype.to_string(),
                        $($field),*
                    }
                }
            }
            impl SockInfo for $name {
                fn get_type_name(&self) -> &str {
                    self.stype.as_str()
                }
                fn get_id(&self) -> u32 {
                    self.id
                }
                $(
                    fn get_description(&$self_ident) -> String {
                        $sock_descr
                    }
                )?
            }
        }
    };
}
pub(crate) use make_simple_sock;
