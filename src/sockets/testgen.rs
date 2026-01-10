use crate::sock::{ComplexSock, SimpleSock, SockBlockCtl, SocketFactory, make_simple_sock, SockDocViewer};
use hex;
use log::debug;
use serde::Deserialize;
use serde_hex::{SerHex, StrictPfx};
use std::cell::RefCell;
use std::io::{Error, ErrorKind};
use std::path::PathBuf;
use std::process;
use std::ptr;
use std::{any::Any, thread, time::Duration};

#[derive(Deserialize, Debug, schemars::JsonSchema)]
#[serde(tag = "type")]
pub enum TestGenTypes {
    /// Constant data production
    #[serde(rename = "static")]
    Static {
        /// Value in hex format
        #[serde(with = "SerHex::<StrictPfx>")]
        #[schemars(with = "String")]
        data: u8,
        /// Length of one iteration pattern
        size: usize,
    },
    /// Sequence from 0..255
    #[serde(rename = "seq")]
    Sequence {
        /// Length of one iteration pattern
        size: usize
    },
    /// Incremental pattern (every next iteration value is one by one higher)
    #[serde(rename = "inc")]
    Increment {
        /// Initial value in hex
        #[serde(with = "SerHex::<StrictPfx>")]
        #[schemars(with = "String")]
        data: u8,
        /// Length of one iteration pattern
        size: usize,
    },
    /// Data blocks pattren
    #[serde(rename = "blocks")]
    Blocks {
        /// Data blocks in hex string format ("00fdea" is 3 blocks pattern)
        #[serde(with = "hex::serde")]
        #[schemars(with = "String")]
        blocks: Vec<u8>,
        /// Length of one block
        block_size: usize,
    },
    /// Text string pattren
    #[serde(rename = "text_str")]
    TextString {
        /// String for production in text format
        data: String
    },
    /// Hex string pattren
    #[serde(rename = "hex_str")]
    HexString {
        /// Hex string data (for example "55ff67aaaa")
        #[serde(with = "hex::serde")]
        #[schemars(with = "String")]
        data: Vec<u8>,
    },
    /// Data from file pattern
    #[serde(rename = "file")]
    File {
        /// Path to file with test pattern
        path: PathBuf
    },
}

#[derive(Deserialize, Debug, schemars::JsonSchema)]
pub struct TestGenConfig {
    /// Test pattern type selection
    pat: TestGenTypes,
    /// Data prosuction cycle in microseconds
    cycle: u64,
    /// Iteration number constrain (optional)
    iter_num: Option<u64>,
}

#[derive(Default)]
pub struct TestGenPrivate {
    pos: usize,
    pattern_size: usize,
    max_iter: Option<u64>,
    curr_iter: u64,
    pattern_priv: Option<Box<dyn Any + Send>>,
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
        _p: &mut Option<Box<dyn Any + Send>>,
        buf: &mut [u8],
        real_size: usize,
        _: usize,
    ) -> std::io::Result<usize> {
        let ret = if let Some(TestGenTypes::Static { data, size: _ }) = cfg.downcast_ref() {
            unsafe {
                std::ptr::write_bytes(buf.as_mut_ptr(), *data, real_size);
            }
            real_size
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
        p: &mut Option<Box<dyn Any + Send>>,
        buf: &mut [u8],
        real_size: usize,
        _: usize,
    ) -> std::io::Result<usize> {
        let ret = if let Some(TestGenTypes::Sequence { size: _ }) = cfg.downcast_ref()
            && let Some(last_data) = p.as_mut().unwrap().downcast_mut::<u8>()
        {
            let mut test_vec = vec![0u8; real_size];
            for (i, el) in test_vec.iter_mut().enumerate() {
                *el = ((i + *last_data as usize) & 0xFF) as u8;
            }
            // Save the last data
            if !test_vec.is_empty() {
                *last_data = ((test_vec[real_size - 1] as usize + 1) & 0xFF) as u8;
            }
            unsafe {
                std::ptr::copy_nonoverlapping(test_vec.as_ptr(), buf.as_mut_ptr(), real_size);
            }
            real_size
        } else {
            return Err(Error::from(ErrorKind::InvalidData));
        };
        Ok(ret)
    }
    fn reset_priv(&self, _p: &mut Option<Box<dyn Any + Send>>) {
        if let Some(last_data) = _p.as_mut().unwrap().downcast_mut::<u8>() {
            *last_data = 0;
        }
    }
}

struct IncrementStrategy;
impl TestPatternStrategy for IncrementStrategy {
    fn read(
        &self,
        cfg: &(dyn Any + Send),
        p: &mut Option<Box<dyn Any + Send>>,
        buf: &mut [u8],
        real_size: usize,
        _: usize,
    ) -> std::io::Result<usize> {
        #[allow(unused_variables)]
        let ret = if let Some(TestGenTypes::Increment { data, size }) = cfg.downcast_ref()
            && let Some(last_data) = p.as_ref().unwrap().downcast_ref::<u8>()
        {
            unsafe {
                std::ptr::write_bytes(buf.as_mut_ptr(), *last_data, real_size);
            }
            real_size
        } else {
            return Err(Error::from(ErrorKind::InvalidData));
        };
        Ok(ret)
    }
    fn reset_priv(&self, _p: &mut Option<Box<dyn Any + Send>>) {
        if let Some(last_data) = _p.as_mut().unwrap().downcast_mut::<u8>() {
            *last_data = ((*last_data as usize + 1) & 0xFF) as u8;
        }
    }
}

struct BlockStrategy;
impl TestPatternStrategy for BlockStrategy {
    fn read(
        &self,
        cfg: &(dyn Any + Send),
        _: &mut Option<Box<dyn Any + Send>>,
        buf: &mut [u8],
        real_size: usize,
        pos: usize,
    ) -> std::io::Result<usize> {
        let ret = if let Some(TestGenTypes::Blocks { blocks, block_size }) = cfg.downcast_ref() {
            let bs = *block_size;
            let mut curr = 0usize;
            // Get start block data, according to current pattern position
            let start = blocks.iter().skip(pos / bs);
            for el in start {
                // Get remaining block size, according to current
                // position
                let chunk = bs - ((pos + curr) % bs);
                // Compare remaining block size with max possible size
                if curr + chunk > real_size {
                    unsafe {
                        ptr::write_bytes(buf.as_mut_ptr().wrapping_add(curr), *el, real_size - curr)
                    };
                    break;
                } else {
                    unsafe { ptr::write_bytes(buf.as_mut_ptr().wrapping_add(curr), *el, chunk) };
                    curr += chunk;
                }
            }
            real_size
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
        _: &mut Option<Box<dyn Any + Send>>,
        buf: &mut [u8],
        real_size: usize,
        pos: usize,
    ) -> std::io::Result<usize> {
        let ret = if let Some(TestGenTypes::TextString { data }) = cfg.downcast_ref() {
            unsafe {
                ptr::copy_nonoverlapping(data.as_ptr().wrapping_add(pos), buf.as_mut_ptr(), real_size);
            }
            real_size
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
        _: &mut Option<Box<dyn Any + Send>>,
        buf: &mut [u8],
        real_size: usize,
        pos: usize,
    ) -> std::io::Result<usize> {
        let ret = if let Some(TestGenTypes::HexString { data }) = cfg.downcast_ref() {
            unsafe {
                ptr::copy_nonoverlapping(data.as_ptr().wrapping_add(pos), buf.as_mut_ptr(), real_size);
            }
            real_size
        } else {
            return Err(Error::from(ErrorKind::InvalidData));
        };
        Ok(ret)
    }
}

struct FileStrategy;
impl TestPatternStrategy for FileStrategy {
    fn read(
            &self,
            _: &(dyn Any + Send),
            p: &mut Option<Box<dyn Any + Send>>,
            buf: &mut [u8],
            real_size: usize,
            pos: usize,
        ) -> std::io::Result<usize> {
        let ret = if let Some(data) = p.as_ref().unwrap().downcast_ref::<String>() {
            unsafe {
                ptr::copy_nonoverlapping(data.as_ptr().wrapping_add(pos), buf.as_mut_ptr(), real_size);
            }
            real_size
        } else {
            return Err(Error::from(ErrorKind::InvalidData));
        };
        Ok(ret)
    }
}

pub trait TestPatternStrategy {
    fn read(
        &self,
        cfg: &(dyn Any + Send),
        p: &mut Option<Box<dyn Any + Send>>,
        buf: &mut [u8],
        real_size: usize,
        pos: usize,
    ) -> std::io::Result<usize>;
    fn reset_priv(&self, _p: &mut Option<Box<dyn Any + Send>>) {}
}

make_simple_sock!(SimpleTestGen {
    config: TestGenConfig,
    pat_cfg: Box<dyn Any + Send>,
    p: RefCell<TestGenPrivate>,
    reader: Box<dyn TestPatternStrategy + Send>,
}, "test-gen");

impl SimpleSock for SimpleTestGen {
    fn read(&self, data: &mut [u8], sz: usize) -> std::io::Result<usize> {
        let mut p = self.p.borrow_mut();
        // Sleep only if pattern starts
        if p.pos == 0 {
            thread::sleep(Duration::from_micros(self.config.cycle));
        }
        // Get real size, according to pattern size, current position of
        // pattern producing & requested size
        let real_size = get_curr_size(p.pattern_size, sz, p.pos);
        let pos = p.pos;
        let ret = self.reader
            .read(self.pat_cfg.as_ref(), &mut p.pattern_priv, data, real_size, pos)?;
        // Update position of pattern producing
        update_pos(&mut p, sz, real_size);
        // End of pattern block
        if p.pos == 0 {
            // Check if iteration constrains were configured
            if let Some(max_iter) = p.max_iter {
                p.curr_iter += 1;
                if p.curr_iter > max_iter {
                    println!("Max iteration limit is reached ({max_iter} iterations)");
                    process::exit(0);
                }
            }
            // Reset private strategy state, if implemented
            self.reader.reset_priv(&mut p.pattern_priv);
        }
        Ok(ret)
    }
    fn write(&self, _: &[u8], _: usize) -> std::io::Result<()> {
        debug!("Socket test-gen unsupports write operation! Skipping...");
        Ok(())
    }
}

impl SockBlockCtl for SimpleTestGen {}

struct TestGenDoc;
impl SockDocViewer for TestGenDoc {
    fn get_full_scheme(&self) -> String {
        let schema = schemars::schema_for!(TestGenConfig);
        serde_json::to_string_pretty(&schema).unwrap()
    }
    fn get_examples(&self) -> String {
        let inc_cfg = "{ \"pat\": { \"type\": \"inc\", \"data\": \"0xf0\", \"size\": 100 }, \"cycle\": 10000 }";
        let hex_str_cfg = "{ \"pat\": { \"type\": \"hex_str\", \"data\": \"1122334455aaddff\" }, \"cycle\": 10000, \"iter_num\": 10 }";
        format!(
            "{}: {}\n{}: {}",
            "Incremantal traffic generation", inc_cfg,
            "Hex string traffic generation (only 10 iterations)", hex_str_cfg
        )
    }
}

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

        let mut p: TestGenPrivate = TestGenPrivate { max_iter: testgen_cfg.iter_num, ..Default::default() };
        let (cb, pat_cfg, p) = match &testgen_cfg.pat {
            TestGenTypes::Static { data, size } => {
                p.pattern_size = *size;
                (
                    Box::new(StaticStrategy) as Box<dyn TestPatternStrategy + Send>,
                    Box::new(TestGenTypes::Static {
                        data: *data,
                        size: *size,
                    }),
                    RefCell::new(p),
                )
            }
            TestGenTypes::Sequence { size } => {
                p.pattern_priv = Some(Box::new(0u8));
                p.pattern_size = *size;
                (
                    Box::new(SequenceStrategy) as Box<dyn TestPatternStrategy + Send>,
                    Box::new(TestGenTypes::Sequence { size: *size }),
                    RefCell::new(p),
                )
            }
            TestGenTypes::Increment { data, size } => {
                p.pattern_priv = Some(Box::new(*data));// Reset private strategy state, if implemented
                p.pattern_size = *size;
                (
                    Box::new(IncrementStrategy) as Box<dyn TestPatternStrategy + Send>,
                    Box::new(TestGenTypes::Increment {
                        data: *data,
                        size: *size,
                    }),
                    RefCell::new(p),
                )
            }
            TestGenTypes::Blocks { blocks, block_size } => {
                p.pattern_size = block_size * blocks.len();
                (
                    Box::new(BlockStrategy) as Box<dyn TestPatternStrategy + Send>,
                    Box::new(TestGenTypes::Blocks {
                        blocks: blocks.clone(),
                        block_size: *block_size,
                    }),
                    RefCell::new(p),
                )
            },
            TestGenTypes::TextString { data } => {
                p.pattern_size = data.len();
                (
                    Box::new(TextStringStrategy) as Box<dyn TestPatternStrategy + Send>,
                    Box::new(TestGenTypes::TextString { data: data.clone() }),
                    RefCell::new(p),
                )
            }
            TestGenTypes::HexString { data } => {
                p.pattern_size = data.len();
                (
                    Box::new(HexStringStrategy) as Box<dyn TestPatternStrategy + Send>,
                    Box::new(TestGenTypes::HexString { data: data.clone() }),
                    RefCell::new(p),
                )
            }
            TestGenTypes::File { path } => {
                let data = std::fs::read_to_string(path)?;
                p.pattern_size = data.len();
                p.pattern_priv = Some(Box::new(data));
                (
                    Box::new(FileStrategy) as Box<dyn TestPatternStrategy + Send>,
                    Box::new(TestGenTypes::File { path: path.clone() }),
                    RefCell::new(p),
                )
            }
        };

        Ok(Box::new(SimpleTestGen::new(testgen_cfg, pat_cfg, p, cb)))
    }
    fn create_doc_viewer(&self) -> Box<dyn SockDocViewer> {
        Box::new(TestGenDoc)
    }
}

mod tests {
    #![allow(unused_imports)]

    use crate::{sock::SocketFactory, sockets::testgen::{TestGenConfig, TestGenFactory}};

    #[test]
    fn parse_config() {
        let cfg =
            "{ \"pat\": { \"type\": \"static\", \"data\": \"0xf0\", \"size\": 10 }, \"cycle\": 1000 }";
        let cfg: TestGenConfig = serde_json::from_str(cfg).unwrap();
        println!("{:?}", cfg);
    }
    #[test]
    fn test_doc_params() {
        println!("{}", TestGenFactory::new().create_doc_viewer().get_full_scheme());
    }
}
