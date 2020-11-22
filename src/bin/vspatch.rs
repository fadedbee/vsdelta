use structopt::StructOpt;
use std::fs::{File, OpenOptions};
use std::io::{SeekFrom, Result};
use vsdelta::common::{OP_CPY, OP_IMM, OP_END};
use std::io::prelude::*;

#[derive(StructOpt)]
struct Cli {
    file_a: String,
    delta_input: String,
    file_b: Option<String>,
}

// little endian
#[inline]
fn u8aletou64(b: [u8; 8]) -> u64 {
    ((b[0] as u64) << 0) +
    ((b[1] as u64) << 8) +
    ((b[2] as u64) << 16) +
    ((b[3] as u64) << 24) +
    ((b[4] as u64) << 32) +
    ((b[5] as u64) << 40) +
    ((b[6] as u64) << 48) +
    ((b[7] as u64) << 56)
}

/* 
 * Copies "num" bytes from src to dst.
 */
fn copy_data(dst: &mut File, src: &mut File, num: u64) -> Result<()> {
    const OP_CPY_CHUNKSIZE: usize = 8;
    const OP_CPY_CHUNKLEN: u64 = OP_CPY_CHUNKSIZE as u64;

    let num_chunks = num / OP_CPY_CHUNKLEN;
    let remainder = num - num_chunks * OP_CPY_CHUNKLEN;

    let mut copybuf = vec![0u8; OP_CPY_CHUNKSIZE];
    for _ in 0..(num / OP_CPY_CHUNKLEN) {
        src.read(&mut copybuf).unwrap();
        dst.write(&copybuf).unwrap();
    }

    let mut copybuf = vec![0u8; remainder as usize];
    src.read(&mut copybuf).unwrap();
    dst.write(&copybuf).unwrap();

    Ok(())
}

fn inplace(file_name: String, delta_name: String) -> Result<()> {
    let mut file = match OpenOptions::new().read(true).write(true).open(&file_name) {
        Ok(file) => file,
        Err(err) => {
            panic!("failed to open {:?}, {:?}", file_name, err);
        }
    };
    let flen = file.metadata().unwrap().len();
    let mut delta = File::open(delta_name)?;
    let nlen = delta.metadata().unwrap().len();

    loop {
        let mut opbuf = [0u8; 1];
        let mut count_buf = [0u8; 8];
        delta.read_exact(&mut opbuf)?;
        let opcode = opbuf[0];

        match opcode {
            OP_CPY => {
                delta.read_exact(&mut count_buf)?;
                let count = u8aletou64(count_buf);
                println!("OP_CPY {:?}", count);
                file.seek(SeekFrom::Current(count as i64))?; // skip
            },
            OP_IMM => {
                delta.read_exact(&mut count_buf)?;
                let count = u8aletou64(count_buf);
                println!("OP_IMM {:?}", count);
                copy_data(&mut file, &mut delta, count)?; // copy data from delta
            },
            OP_END => {
                println!("OP_END");
                break;
            },
            _ => {
                println!("error: bad format");
            }
        }
    }

    Ok(())
}

fn external(src_name: String, delta_name: String, dst_name: String) -> Result<()> {
    let mut src = File::open(src_name)?;
    let slen = src.metadata().unwrap().len();
    let mut delta = File::open(delta_name)?;
    let nlen = delta.metadata().unwrap().len();
    let mut dst = File::create(dst_name)?;

    loop {
        let mut opbuf = [0u8; 1];
        let mut count_buf = [0u8; 8];
        delta.read_exact(&mut opbuf)?;
        let opcode = opbuf[0];

        match opcode {
            OP_CPY => {
                delta.read_exact(&mut count_buf)?;
                let count = u8aletou64(count_buf);
                println!("OP_CPY {:?}", count);
                copy_data(&mut dst, &mut src, count)?; // copy data from src
            },
            OP_IMM => {
                delta.read_exact(&mut count_buf)?;
                let count = u8aletou64(count_buf);
                println!("OP_IMM {:?}", count);
                src.seek(SeekFrom::Current(count as i64))?; // skip data in src
                copy_data(&mut dst, &mut delta, count)?; // copy data from delta
            },
            OP_END => {
                println!("OP_END");
                break;
            },
            _ => {
                println!("error: bad format");
            }
        }
    }

    Ok(())
}

fn main() -> Result<()> {
	let args = Cli::from_args();

    match args.file_b {
        None => {
            inplace(args.file_a, args.delta_input)?;
        }
        Some(file_b) => {
            external(args.file_a, args.delta_input, file_b)?;
        }
    }

	Result::Ok(())
}
