//! Polymesh CLI binary.
#![warn(missing_docs)]
#![warn(unused_extern_crates)]

mod chain_spec;
#[macro_use]
mod service;
mod cli;
mod command;

fn main() -> command::Result<()> {
    command::run()
}
