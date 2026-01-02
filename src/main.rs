use std::io;

mod sockets;
mod sock;
mod serde_helpers;
mod test_helpers;
mod args;
mod modes;

use crate::args::PolySockArgs;

fn main() -> io::Result<()> {
    let mut command = PolySockArgs::get_scenario();
    command.execute();
    Ok(())
}
