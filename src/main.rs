/*
 * Copyright (c) 2026 Ilya Shishov
 * Licensed under the MIT License.
 * See the LICENSE file in the project root for full license information.
 */

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
