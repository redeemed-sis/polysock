use std::{collections::HashMap, io::Result, mem::size_of};

/// A simple socket trait providing basic read/write operations.
pub trait SimpleSock {
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
    fn set_block(&self, is_block: bool) -> Result<()>;
}

pub trait SimpleSockBlock: SimpleSock + SockBlockCtl {}

// Any type that impl SimpleSock & SockBlockCtl automatically
// implements SimpleSockBlock
impl<T: SimpleSock + SockBlockCtl> SimpleSockBlock for T {}

pub trait SocketFactory {
    /// Creates a new SimpleSock instance with the given parameters.
    fn create_sock(&self, params: HashMap<String, String>) -> Result<Box<dyn SimpleSockBlock>>;
    fn create_sock_blockctl(&self, params: HashMap<String, String>, is_blocking: bool) -> Result<Box<dyn SimpleSockBlock>> {
        let soc = self.create_sock(params)?;
        soc.set_block(is_blocking)?;
        Ok(soc)
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
            let chunk = self.get_simple_sock().read(
                &mut buffer[bytes_read..],
                bytes_needed - bytes_read,
            )?;
            if chunk == 0 {
                break;
            }
            bytes_read += chunk;
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
            if chunk.is_empty() {
                break;
            }
            result.extend(chunk);
        }

        Ok(result)
    }
}
