use clap::error::ErrorKind;
use derive_builder::Builder;

use crate::sock::{SocketFactory, SocketManager, SocketParams};
use std::io::Error;
use std::process;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::{io, sync::atomic::AtomicBool, thread::JoinHandle};

pub struct OnelinerMode {
    f_factory: Box<dyn SocketFactory>,
    to_factory: Box<dyn SocketFactory>,
    params: OnelinerModeParams,
    handle1: Option<JoinHandle<io::Result<()>>>,
    handle2: Option<JoinHandle<io::Result<()>>>,
    run_ctl: Option<Arc<AtomicBool>>,
}

#[derive(Builder)]
pub struct OnelinerModeParams {
    #[builder(default)]
    f_params: SocketParams,
    #[builder(default)]
    to_params: SocketParams,
    #[builder(default = false)]
    bidir: bool,
    #[builder(default = true)]
    blocking: bool,
}

impl OnelinerMode {
    pub fn new(
        fdev: Box<dyn SocketFactory>,
        todev: Box<dyn SocketFactory>,
        params: OnelinerModeParams,
    ) -> Self {
        Self {
            f_factory: fdev,
            to_factory: todev,
            params,
            handle1: None,
            handle2: None,
            run_ctl: None,
        }
    }
    pub fn start(&mut self) -> io::Result<()> {
        let manager = SocketManager::new(&self.f_factory, &self.to_factory);
        let params = &self.params;
        if !params.bidir {
            let (h, r) = manager.bind_unidirectional(
                &params.f_params,
                &params.to_params,
                params.blocking,
            )?;
            self.handle1 = Some(h);
            self.run_ctl = Some(r);
        } else {
            let (h1, h2, r) = manager.bind_bidirectional(&params.f_params, &params.to_params)?;
            self.handle1 = Some(h1);
            self.handle2 = Some(h2);
            self.run_ctl = Some(r);
        }
        Ok(())
    }
    pub fn wait(&mut self) -> io::Result<()> {
        let ret = if let Some(handle1) = self.handle1.take() {
            handle1.join().unwrap_or_else(|_| {eprintln!("Unexpected error while joining thread!"); process::exit(1)})
        } else {
            return Err(io::Error::from(io::ErrorKind::InvalidData));
        };
        if let Err(e) = ret {
            return Err(e)
        }
        let ret = if let Some(handle2) = self.handle2.take() {
            handle2.join().unwrap_or_else(|_| {eprintln!("Unexpected error while joining thread!"); process::exit(1)})
        } else {
            return Ok(());
        };
        if let Err(e) = ret {
            return Err(e)
        }

        Ok(())
    }
    pub fn stop(&mut self) -> io::Result<()> {
        if let Some(run_ctl) = self.run_ctl.take() {
            run_ctl.store(false, Ordering::Relaxed);
        } else {
            return Err(io::Error::from(io::ErrorKind::InvalidData));
        }
        Ok(())
    }
}

pub struct OnelinerModeCommand {
    mode: OnelinerMode,
}

impl OnelinerModeCommand {
    pub fn new(mode: OnelinerMode) -> Self {
        Self { mode }
    }
}

impl super::Command for OnelinerModeCommand {
    fn execute(&mut self) {
        match self.mode.start() {
            Err(err) => {
                eprintln!("Error during start oneliner task: {err}");
                process::exit(1);
            }
            Ok(_) => {
                if let Err(e) = self.mode.wait() {
                    eprintln!("Thread finished with error: {e}");
                    process::exit(1);
                }
            }
        }
    }
}
