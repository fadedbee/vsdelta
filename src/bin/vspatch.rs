use structopt::StructOpt;
use std::fs::File;
use std::io::{SeekFrom, Result};
use std::cmp::min;
use std::io::prelude::*;

#[derive(StructOpt)]
struct Cli {
    old_file: String,
    delta_file: String,
    new_file: Option<String>,
}

fn main() -> Result<()> {
	let args = Cli::from_args();

    println!("not implemented yet...");

	Result::Ok(())
}
