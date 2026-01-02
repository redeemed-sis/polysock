use derive_builder::Builder;

use crate::sock::{SocketFactory, SocketManager, SocketParams};
use std::sync::Arc;
use std::{io, sync::atomic::AtomicBool, thread::JoinHandle};
use std::process;

pub struct OnelinerMode {
    f_factory: Box<dyn SocketFactory>,
    to_factory: Box<dyn SocketFactory>,
    params: OnelinerModeParams,
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
        }
    }
    pub fn start(&mut self) -> io::Result<(JoinHandle<io::Result<()>>, Arc<AtomicBool>)> {
        let manager = SocketManager::new(&self.f_factory, &self.to_factory);
        let params = &self.params;
        manager.bound_inout(&params.f_params, &params.to_params, params.blocking)
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
            Ok((handle, _)) => {
                if let Err(e) = handle.join().unwrap() {
                    eprintln!("Oneliner task is finished with error: {e}");
                    process::exit(1);
                }
            }
        }
    }
}
