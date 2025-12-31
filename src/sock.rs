use std::{collections::HashMap, io::Result};

pub trait SimpleSock {
    fn open(&self) -> Result<()> {
        Ok(())
    }
    fn close(&self) {}
    fn read(&self, data: &mut [u8], sz: usize) -> Result<usize>;
    fn write(&self, data: &[u8], sz: usize) -> Result<()>;
}

pub trait Socket {
    fn get_simple_sock(&self) -> &dyn SimpleSock;
    fn set_simple_sock(&mut self, simple_sock: Box<dyn SimpleSock>);
    fn create_sock(&self, params: HashMap<String, String>) -> Result<Box<dyn SimpleSock>>;
    fn generic_read<T: Default + Clone>(&self, sz: usize) -> Result<Vec<T>> {
        let mut raw_data = vec![0_u8; size_of::<T>() * sz];
        let mut count = 0_usize;

        while count < raw_data.len() {
            let bytes_read = self
                .get_simple_sock()
                .read(raw_data[count..].as_mut(), size_of::<T>() * sz - count)?;
            if bytes_read == 0 {
                break;
            }
            count += bytes_read;
        }

        let mut user_data: Vec<T> = vec![T::default(); sz];

        unsafe {
            (user_data.as_mut_ptr() as *mut u8).copy_from(raw_data.as_ptr(), count);
        }

        Ok(user_data)
    }
    fn generic_write<T>(&self, data: &[T], sz: usize) -> Result<()> {
        let mut inner_data = vec![0u8; sz * size_of::<T>()];
        unsafe {
            inner_data
                .as_mut_ptr()
                .copy_from(data.as_ptr() as *const u8, sz * size_of::<T>());
        }
        self.get_simple_sock()
            .write(inner_data.as_slice(), sz * size_of::<T>())?;
        Ok(())
    }
    fn read_all<T: Default + Clone>(&self) -> Result<Vec<T>> {
        const ITER_LEN: usize = std::usize::MAX;
        let mut user_data = Vec::new();
        loop {
            let mut iter_data = self.generic_read(ITER_LEN)?;
            if iter_data.is_empty() {
                break;
            }
            user_data.append(iter_data.as_mut());
        }
        Ok(user_data)
    }
}
