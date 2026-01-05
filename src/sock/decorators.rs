use super::{ComplexSock, SockBlockCtl, SockInfo, SocketParams, SocketFactory, SimpleSock};
use std::io::Result;
use pretty_hex::{self, PrettyHex};

macro_rules! socket_decorator {
    ($name: ident) => {
        pub struct $name {
            sock: Box<dyn ComplexSock>,
        }
        impl $name {
            pub fn new(sock: Box<dyn ComplexSock>) -> Box<dyn ComplexSock> {
                Box::new(Self { sock })
            }
        }
        impl SockBlockCtl for $name {
            fn set_block(&mut self, is_blocking: bool) -> Result<()> {
                self.sock.set_block(is_blocking)
            }
        }
        impl SockInfo for $name {
            fn get_type_name(&self) -> &str {
                self.sock.get_type_name()
            }
            fn get_id(&self) -> u32 {
                self.sock.get_id()
            }
            fn get_description(&self) -> String {
                self.sock.get_description()
            }
        }
        paste::paste! {
            pub struct [< $name Factory >] {
                factory: Box<dyn SocketFactory>,
            }
            impl [< $name Factory >] {
                pub fn new(factory: Box<dyn SocketFactory>) -> Box<dyn SocketFactory> {
                    Box::new(Self { factory })
                }
            }
            impl SocketFactory for [< $name Factory >] {
                fn create_sock(&self, params: SocketParams) -> Result<Box<dyn ComplexSock>> {
                    let res = self.factory.create_sock(params);
                    if let Ok(sock) = res {
                        return Ok($name::new(sock));
                    }
                    res
                }
            }
        }
    };
}

macro_rules! decorator_openclose_default {
    () => {
        fn open(&mut self) -> Result<()> {
            self.sock.open()
        }
        fn close(&mut self) {
            self.sock.close();
        }
    };
}

socket_decorator!(TraceInfoDecorator);

impl SimpleSock for TraceInfoDecorator {
    fn read(&self, data: &mut [u8], sz: usize) -> Result<usize> {
        let sock = self.sock.as_ref();
        let res = sock.read(data, sz);
        if let Ok(sz) = res {
            if sz > 0 {
                println!("Data is received from: {}", sock.get_description());
            }
        }
        res
    }
    fn write(&self, data: &[u8], sz: usize) -> Result<()> {
        let sock = self.sock.as_ref();
        let res = sock.write(data, sz);
        if sz > 0 {
            println!("Data is transered to: {}", sock.get_description());
        }
        res
    }
    fn open(&mut self) -> Result<()> {
        let sock = self.sock.as_mut();
        println!("Socket is opened: {}", sock.get_description());
        sock.open()
    }
    fn close(&mut self) {
        let sock = self.sock.as_mut();
        println!("Socket is closed: {}", sock.get_description());
        sock.close()
    }
}

socket_decorator!(TraceRawDecorator);

impl SimpleSock for TraceRawDecorator {
    fn read(&self, data: &mut [u8], sz: usize) -> Result<usize> {
        let res = self.sock.read(data, sz);
        if let Ok(sz) = res {
            if sz > 0 {
                println!("Data is received: {:?}", data[..sz].as_ref());
            }
        }
        res
    }
    fn write(&self, data: &[u8], sz: usize) -> Result<()> {
        let sock = self.sock.as_ref();
        let res = sock.write(data, sz);
        if sz > 0 {
            println!("Data is written: {:?}", data[..sz].as_ref());
        }
        res
    }
    decorator_openclose_default!();
}

socket_decorator!(TraceCanonicalDecorator);

impl SimpleSock for TraceCanonicalDecorator {
    fn read(&self, data: &mut [u8], sz: usize) -> Result<usize> {
        let res = self.sock.read(data, sz);
        if let Ok(sz) = res {
            if sz > 0 {
                println!("Received data (canonical format):\n {:?}", data[..sz].hex_dump());
            }
        }
        res
    }
    fn write(&self, data: &[u8], sz: usize) -> Result<()> {
        let sock = self.sock.as_ref();
        let res = sock.write(data, sz);
        if sz > 0 {
            println!("Written data (canonical format):\n{:?}", data[..sz].hex_dump());
        }
        res
    }
    decorator_openclose_default!();
}
