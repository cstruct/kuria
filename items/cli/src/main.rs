//! `kuria`: see the crate's README.

use std::process::ExitCode;

use clap::Parser as _;

fn main() -> ExitCode {
    let cli = kuria_cli::args::Cli::parse();
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("a Tokio runtime")
        .block_on(kuria_cli::run(cli))
}
