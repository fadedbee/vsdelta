use structopt::StructOpt;
use std::fs::File;
use std::io::{SeekFrom, Result};
use vsdelta::common::*;
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
    const OP_SKIP_CHUNKSIZE: usize = 8;
    const OP_SKIP_CHUNKLEN: u64 = OP_SKIP_CHUNKSIZE as u64;

    let num_chunks = num / OP_SKIP_CHUNKLEN;
    let remainder = num - num_chunks * OP_SKIP_CHUNKLEN;

    let mut copybuf = vec![0u8; OP_SKIP_CHUNKSIZE];
    for _ in 0..(num / OP_SKIP_CHUNKLEN) {
        src.read(&mut copybuf).unwrap();
        dst.write(&copybuf).unwrap();
    }

    let mut copybuf = vec![0u8; remainder as usize];
    src.read(&mut copybuf).unwrap();
    dst.write(&copybuf).unwrap();

    Ok(())
}

fn read_magic(delta: &mut File) -> Result<()> {
    let mut buf = [0u8; 7];
    delta.read_exact(&mut buf)?;
    
    let magic = match std::str::from_utf8(&buf) {
        Ok(v) => v,
        Err(e) => panic!("Invalid UTF-8 sequence: {}", e),
    };
    
    if magic != "vsdelta" {
        panic!("Not a vsdelta file.");
    }
    Ok(())
}

fn op_ver(delta: &mut File) -> Result<()> {
    let mut ver = [0u8; 3];
    delta.read_exact(&mut ver)?;
    if ver[0] != 0 {
        panic!("Incompatible version.");
    }
    Ok(())
}

fn op_len_a(delta: &mut File, alen: u64)-> Result<()>  {
    let mut lenbuf = [0u8; 8];
    delta.read_exact(&mut lenbuf)?;
    let len = u8aletou64(lenbuf);
    if len != alen {
        panic!("This delta expects file_a to be {:?} bytes long, not {:?} bytes.", alen, len);
    }
    Ok(())
}

fn op_sha256_a(delta: &mut File) -> Result<()> {
    let mut shabuf = [0u8; 32];
    delta.read_exact(&mut shabuf)?;
    // FIXME: check sha256
    Ok(())
}

fn op_sha256_b(delta: &mut File) -> Result<()> {
    let mut shabuf = [0u8; 32];
    delta.read_exact(&mut shabuf)?;
    // FIXME: check sha256
    Ok(())
}

fn main() -> Result<()> {
    let args = Cli::from_args();
    
    let mut file_a = File::open(args.file_a)?;
    let alen = file_a.metadata().unwrap().len();
    let mut delta = File::open(args.delta_input)?;
    let mut opt_file_b = match args.file_b {
        Some(file_b) => Some(File::create(file_b)?),
        None => None
    };

    read_magic(&mut delta)?;

    loop {
        let mut opbuf = [0u8; 1];
        let mut count_buf = [0u8; 8];
        delta.read_exact(&mut opbuf)?;
        let opcode = opbuf[0];

        match opcode {
            OP_VER => {
                op_ver(&mut delta)?;
            }
            OP_LEN_A => {
                op_len_a(&mut delta, alen)?;
            }
            OP_SHA256_A => {
                op_sha256_a(&mut delta)?;
            }
            OP_SKIP => {
                delta.read_exact(&mut count_buf)?;
                let count = u8aletou64(count_buf);
                println!("OP_SKIP {:?}", count);
                match opt_file_b {
                    Some(ref mut file_b) => {
                        copy_data(file_b, &mut file_a, count)?; // copy data from file_a
                    },
                    None => {
                        file_a.seek(SeekFrom::Current(count as i64))?; // skip, nothing to do
                    }
                }
            }
            OP_DIFF => {
                delta.read_exact(&mut count_buf)?;
                let count = u8aletou64(count_buf);
                println!("OP_DIFF {:?}", count);
                match opt_file_b {
                    Some(ref mut file_b) => {
                        file_a.seek(SeekFrom::Current(count as i64))?; // skip data in file_a
                        copy_data(file_b, &mut delta, count)?; // copy data from delta
                    },
                    None => {
                        copy_data(&mut file_a, &mut delta, count)?; // copy data from delta
                    }
                }
            }
            OP_SHA256_B => {
                op_sha256_b(&mut delta)?;
            }
            OP_END => {
                break;
            }
            _ => {
                panic!("error: bad format");
            }
        }
    }

	Result::Ok(())
}
