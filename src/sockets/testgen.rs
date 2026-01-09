use crate::sock::{ComplexSock, SimpleSock, SockBlockCtl, SocketFactory, make_simple_sock};
use hex;
use log::debug;
use serde::Deserialize;
use serde_hex::{SerHex, StrictPfx};
use std::cell::RefCell;
use std::io::{Error, ErrorKind};
use std::ptr;
use std::{any::Any, thread, time::Duration};

#[derive(Deserialize, Debug)]
#[serde(tag = "type")]
pub enum TestGenTypes {
    #[serde(rename = "static")]
    Static {
        #[serde(with = "SerHex::<StrictPfx>")]
        data: u8,
        size: usize,
    },
    #[serde(rename = "seq")]
    Sequence { size: usize },
    #[serde(rename = "inc")]
    Increment {
        #[serde(with = "SerHex::<StrictPfx>")]
        data: u8,
        size: usize,
    },
    #[serde(rename = "blocks")]
    Blocks {
        #[serde(with = "hex::serde")]
        blocks: Vec<u8>,
        block_size: usize,
    },
    #[serde(rename = "text_str")]
    TextString { data: String },
    #[serde(rename = "hex_str")]
    HexString {
        #[serde(with = "hex::serde")]
        data: Vec<u8>,
    },
}

pub struct TestGenPrivate {
    pos: usize,
    last_data: u8,
}

fn get_curr_size(pattern_size: usize, req_size: usize, pos: usize) -> usize {
    // return data size according to requested
    // transaction size, data pattern size and
    // current position
    if pattern_size - pos < req_size {
        pattern_size - pos
    } else {
        req_size
    }
}

fn update_pos(p: &mut TestGenPrivate, req: usize, ret: usize) {
    // Update current pos, according to data left.
    // If s < sz it means that pattern data is completely
    // obtained by Socket client
    if ret < req { p.pos = 0 } else { p.pos += req }
}

struct StaticStrategy;
impl TestPatternStrategy for StaticStrategy {
    fn read(
        &self,
        cfg: &(dyn Any + Send),
        p: &mut TestGenPrivate,
        buf: &mut [u8],
        sz: usize,
    ) -> std::io::Result<usize> {
        let ret = if let Some(TestGenTypes::Static { data, size }) = cfg.downcast_ref() {
            // Get needed size
            let ret = get_curr_size(*size, sz, p.pos);
            unsafe {
                std::ptr::write_bytes(buf.as_mut_ptr(), *data, ret);
            }
            // Update position in private
            update_pos(p, sz, ret);
            ret
        } else {
            return Err(Error::from(ErrorKind::InvalidData));
        };
        Ok(ret)
    }
}

struct SequenceStrategy;
impl TestPatternStrategy for SequenceStrategy {
    fn read(
        &self,
        cfg: &(dyn Any + Send),
        p: &mut TestGenPrivate,
        buf: &mut [u8],
        sz: usize,
    ) -> std::io::Result<usize> {
        let ret = if let Some(TestGenTypes::Sequence { size }) = cfg.downcast_ref() {
            // Get needed size
            let ret = get_curr_size(*size, sz, p.pos);
            let mut test_vec = vec![0u8; ret];
            for (i, el) in test_vec.iter_mut().enumerate() {
                *el = ((i + p.last_data as usize) & 0xFF) as u8;
            }
            // Save the last data
            if !test_vec.is_empty() {
                p.last_data = ((test_vec[ret - 1] as usize + 1) & 0xFF) as u8;
            }
            unsafe {
                std::ptr::copy_nonoverlapping(test_vec.as_ptr(), buf.as_mut_ptr(), ret);
            }
            // Update position in private
            update_pos(p, sz, ret);
            // If pos is zero, reset also previous data
            if p.pos == 0 {
                p.last_data = 0;
            }
            ret
        } else {
            return Err(Error::from(ErrorKind::InvalidData));
        };
        Ok(ret)
    }
}

struct IncrementStrategy;
impl TestPatternStrategy for IncrementStrategy {
    fn read(
        &self,
        cfg: &(dyn Any + Send),
        p: &mut TestGenPrivate,
        buf: &mut [u8],
        sz: usize,
    ) -> std::io::Result<usize> {
        #[allow(unused_variables)]
        let ret = if let Some(TestGenTypes::Increment { data, size }) = cfg.downcast_ref() {
            // Get needed size
            let ret = get_curr_size(*size, sz, p.pos);
            unsafe {
                std::ptr::write_bytes(buf.as_mut_ptr(), p.last_data, ret);
            }
            // Update position in private
            update_pos(p, sz, ret);
            if p.pos == 0 {
                p.last_data = ((p.last_data as usize + 1) & 0xFF) as u8;
            }
            ret
        } else {
            return Err(Error::from(ErrorKind::InvalidData));
        };
        Ok(ret)
    }
}

struct BlockStrategy;
impl TestPatternStrategy for BlockStrategy {
    fn read(
        &self,
        cfg: &(dyn Any + Send),
        p: &mut TestGenPrivate,
        data: &mut [u8],
        sz: usize,
    ) -> std::io::Result<usize> {
        let ret = if let Some(TestGenTypes::Blocks { blocks, block_size }) = cfg.downcast_ref() {
            let bs = *block_size;
            let all_size = blocks.len() * bs;
            // Get real size, which be applied to client vector
            let ret = get_curr_size(all_size, sz, p.pos);
            let mut curr = 0usize;
            // Get start block data, according to current pattern position
            let start = blocks.iter().skip(p.pos / bs);
            for el in start {
                // Get remaining block size, according to current
                // position
                let chunk = bs - ((p.pos + curr) % bs);
                // Compare remaining block size with max possible size
                if curr + chunk > ret {
                    unsafe {
                        ptr::write_bytes(data.as_mut_ptr().wrapping_add(curr), *el, ret - curr)
                    };
                    break;
                } else {
                    unsafe { ptr::write_bytes(data.as_mut_ptr().wrapping_add(curr), *el, chunk) };
                    curr += chunk;
                }
            }
            // Update position, it resets if
            // ret < requested size
            update_pos(p, sz, ret);
            ret
        } else {
            return Err(Error::from(ErrorKind::InvalidData));
        };
        Ok(ret)
    }
}

struct TextStringStrategy;
impl TestPatternStrategy for TextStringStrategy {
    fn read(
        &self,
        cfg: &(dyn Any + Send),
        p: &mut TestGenPrivate,
        buf: &mut [u8],
        sz: usize,
    ) -> std::io::Result<usize> {
        let ret = if let Some(TestGenTypes::TextString { data }) = cfg.downcast_ref() {
            let pattern_size = data.len();
            // Get needed size
            let ret = get_curr_size(pattern_size, sz, p.pos);
            unsafe {
                ptr::copy_nonoverlapping(data.as_ptr().wrapping_add(p.pos), buf.as_mut_ptr(), ret);
            }
            // Update position in private
            update_pos(p, sz, ret);
            ret
        } else {
            return Err(Error::from(ErrorKind::InvalidData));
        };
        Ok(ret)
    }
}

struct HexStringStrategy;
impl TestPatternStrategy for HexStringStrategy {
    fn read(
        &self,
        cfg: &(dyn Any + Send),
        p: &mut TestGenPrivate,
        buf: &mut [u8],
        sz: usize,
    ) -> std::io::Result<usize> {
        let ret = if let Some(TestGenTypes::HexString { data }) = cfg.downcast_ref() {
            let pattern_size = data.len();
            // Get needed size
            let ret = get_curr_size(pattern_size, sz, p.pos);
            unsafe {
                ptr::copy_nonoverlapping(data.as_ptr().wrapping_add(p.pos), buf.as_mut_ptr(), ret);
            }
            // Update position in private
            update_pos(p, sz, ret);
            ret
        } else {
            return Err(Error::from(ErrorKind::InvalidData));
        };
        Ok(ret)
    }
}

#[derive(Deserialize, Debug)]
pub struct TestGenConfig {
    pat: TestGenTypes,
    cycle: u64,
}

pub trait TestPatternStrategy {
    fn read(
        &self,
        cfg: &(dyn Any + Send),
        p: &mut TestGenPrivate,
        data: &mut [u8],
        sz: usize,
    ) -> std::io::Result<usize>;
}

make_simple_sock!(SimpleTestGen {
    config: TestGenConfig,
    pat_cfg: Box<dyn Any + Send>,
    p: RefCell<TestGenPrivate>,
    reader: Box<dyn TestPatternStrategy + Send>,
}, "test-gen");

impl SimpleSock for SimpleTestGen {
    fn read(&self, data: &mut [u8], sz: usize) -> std::io::Result<usize> {
        // Sleep only if pattern starts
        if self.p.borrow().pos == 0 {
            thread::sleep(Duration::from_micros(self.config.cycle));
        }
        self.reader
            .read(self.pat_cfg.as_ref(), &mut self.p.borrow_mut(), data, sz)
    }
    fn write(&self, _: &[u8], _: usize) -> std::io::Result<()> {
        debug!("Socket test-gen unsupports write operation! Skipping...");
        Ok(())
    }
}

impl SockBlockCtl for SimpleTestGen {}

pub struct TestGenFactory;

impl TestGenFactory {
    pub fn new() -> Self {
        Self
    }
}

impl SocketFactory for TestGenFactory {
    fn create_sock(
        &self,
        params: crate::sock::SocketParams,
    ) -> std::io::Result<Box<dyn ComplexSock>> {
        // Deserialize to TestGenConfig
        let testgen_cfg: TestGenConfig = serde_json::from_str(params.as_str()).map_err(|e| {
            eprintln!("{e}");
            Error::new(ErrorKind::InvalidInput, "Invalid test-gen configuration")
        })?;

        let mut p = TestGenPrivate {
            pos: 0,
            last_data: 0,
        };
        let (cb, pat_cfg, p) = match &testgen_cfg.pat {
            TestGenTypes::Static { data, size } => (
                Box::new(StaticStrategy) as Box<dyn TestPatternStrategy + Send>,
                Box::new(TestGenTypes::Static {
                    data: *data,
                    size: *size,
                }),
                RefCell::new(p),
            ),
            TestGenTypes::Sequence { size } => (
                Box::new(SequenceStrategy) as Box<dyn TestPatternStrategy + Send>,
                Box::new(TestGenTypes::Sequence { size: *size }),
                RefCell::new(p),
            ),
            TestGenTypes::Increment { data, size } => {
                p.last_data = *data;
                (
                    Box::new(IncrementStrategy) as Box<dyn TestPatternStrategy + Send>,
                    Box::new(TestGenTypes::Increment {
                        data: *data,
                        size: *size,
                    }),
                    RefCell::new(p),
                )
            }
            TestGenTypes::Blocks { blocks, block_size } => (
                Box::new(BlockStrategy) as Box<dyn TestPatternStrategy + Send>,
                Box::new(TestGenTypes::Blocks {
                    blocks: blocks.clone(),
                    block_size: *block_size,
                }),
                RefCell::new(p),
            ),
            TestGenTypes::TextString { data } => (
                Box::new(TextStringStrategy) as Box<dyn TestPatternStrategy + Send>,
                Box::new(TestGenTypes::TextString { data: data.clone() }),
                RefCell::new(p),
            ),
            TestGenTypes::HexString { data } => (
                Box::new(HexStringStrategy) as Box<dyn TestPatternStrategy + Send>,
                Box::new(TestGenTypes::HexString { data: data.clone() }),
                RefCell::new(p),
            ),
        };

        Ok(Box::new(SimpleTestGen::new(testgen_cfg, pat_cfg, p, cb)))
    }
}

mod tests {
    #![allow(unused_imports)]

    use crate::sockets::testgen::TestGenConfig;

    #[test]
    fn parse_config() {
        let cfg =
            "{ \"pat\": { \"type\": \"static\", \"data\": 100, \"size\": 10 }, \"cycle\": 1000 }";
        let cfg: TestGenConfig = serde_json::from_str(cfg).unwrap();
        println!("{:?}", cfg);
    }
}
