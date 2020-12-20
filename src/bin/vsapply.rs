use structopt::StructOpt;
use std::fs::{File, OpenOptions};
use std::io::{SeekFrom};
use vsdelta::common::*;
use std::io::prelude::*;
use std::panic;
use anyhow::{Context, Result};

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
    const OP_SKIP_CHUNKSIZE: usize = 1024 * 1024;
    const OP_SKIP_CHUNKLEN: u64 = OP_SKIP_CHUNKSIZE as u64;

    let num_chunks = num / OP_SKIP_CHUNKLEN;
    let remainder = num - num_chunks * OP_SKIP_CHUNKLEN;

    let mut copybuf = vec![0xFFu8; OP_SKIP_CHUNKSIZE];
    for _ in 0..(num / OP_SKIP_CHUNKLEN) {
        //let pos = src.seek(SeekFrom::Current(0)).unwrap();
        //println!("pos: {:?}", pos);
        src.read_exact(&mut copybuf).unwrap();
        //println!("copybuf {:X?}", copybuf);
        dst.write(&copybuf).unwrap();
    }

    let mut copybuf = vec![0u8; remainder as usize];
    src.read_exact(&mut copybuf).unwrap();
    //println!("copybuf {:X?}", copybuf);
    dst.write(&copybuf).unwrap();

    Ok(())
}

fn is_zero(buf: &Vec<u8>) -> bool {
    for byte in buf.into_iter() {
        if *byte != 0 {
            return false;
        }
    }
    return true;
}

/* 
 * Copies "num" bytes from src to dst.
 * 
 * Skips blocks of zeros by seeking forwards, creating a sparse file.
 */
fn sparse_copy_data(dst: &mut File, src: &mut File, num: u64) -> Result<()> {
    const OP_SKIP_CHUNKSIZE: usize = 4096;
    const OP_SKIP_CHUNKLEN: u64 = OP_SKIP_CHUNKSIZE as u64;

    let num_chunks = num / OP_SKIP_CHUNKLEN;
    let remainder = num - num_chunks * OP_SKIP_CHUNKLEN;

    let mut copybuf = vec![0xFFu8; OP_SKIP_CHUNKSIZE];
    for _ in 0..(num / OP_SKIP_CHUNKLEN) {
        //let pos = src.seek(SeekFrom::Current(0)).unwrap();
        src.read_exact(&mut copybuf).context("Error reading from source.")?;
        if is_zero(&copybuf) {
            //println!("pos: {:?} (zeros)", pos);
            dst.seek(SeekFrom::Current(OP_SKIP_CHUNKLEN as i64)).context("Error seeking in destination.")?;
        } else {
            //println!("pos: {:?}", pos);
            //println!("copybuf {:X?}", copybuf);
            dst.write(&copybuf).context("Error writing to destination.")?;
        }
    }

    let mut copybuf = vec![0u8; remainder as usize];
    src.read_exact(&mut copybuf).context("Error reading final block from source.")?;
    if is_zero(&copybuf) {
        dst.seek(SeekFrom::Current(remainder as i64)).context("Error seeking in final block of destination")?;
    } else {
        //println!("copybuf {:X?}", copybuf);
        dst.write(&copybuf).context("Error writing to desination in final block.")?;
    }

    // TODO: these three lines only need to be executed if the last thing to happen was a dst.seek() after an is_zero.
    let dst_pos = dst.seek(SeekFrom::Current(0)).context("Error seeking to current position in desintation.")?;
    //println!("dst_pos: {:?} (zeros)", dst_pos);
    dst.set_len(dst_pos).context("Error setting length of destination.")?;

    Ok(())
}

fn read_magic(delta: &mut File) -> Result<()> {
    let mut buf = [0u8; 7];
    delta.read_exact(&mut buf).context("Error reading bytes.")?;
    
    let magic = match std::str::from_utf8(&buf) {
        Ok(v) => v,
        // TODO: don't panic, find a way of returning an error.
        Err(e) => panic!("Invalid UTF-8 sequence: {}", e),
    };
    
    if magic != "vsdelta" {
        // TODO: don't panic, find a way of returning an error.
        panic!("Not a vsdelta file.");
    }
    Ok(())
}

fn op_ver(delta: &mut File) -> Result<()> {
    let mut ver = [0u8; 3];
    delta.read_exact(&mut ver).context("Error reading version bytes.")?;
    if ver[0] != 0 {
        panic!("Incompatible version.");
    }
    // TODO: don't panic, find a way of returning an error.
    //println!("OP_VER {:?}.{:?}.{:?}", ver[0], ver[1], ver[2]);
    Ok(())
}

fn op_len_a(delta: &mut File, alen: u64)-> Result<()>  {
    let mut lenbuf = [0u8; 8];
    delta.read_exact(&mut lenbuf).context("Error reading expected length of file_a.")?;
    let len = u8aletou64(lenbuf);
    //println!("OP_LEN_A {:?}", len);
    if len != alen {
        // TODO: don't panic, find a way of returning an error.
        panic!("This delta expects file_a to be {:?} bytes long, not {:?} bytes.", len, alen);
    }
    Ok(())
}

fn op_hash_a(delta: &mut File, file_a: &mut File, alen: u64) -> Result<()> {
    let mut hashbuf = [0u8; 32];
    delta.read_exact(&mut hashbuf).context("Error reading expected hash of file_a.")?;
    //println!("OP_HASH_A {:02X?}", hashbuf);
    let hash = hash_file(file_a, alen)?;
    if hash != hashbuf {
        // TODO: don't panic, find a way of returning an error.
        panic!("This delta expects file_a's hash to be {:X?}, not {:X?}.", hashbuf, hash);
    };
    Ok(())
}

fn op_len_b(delta: &mut File, file: &mut File)-> Result<()>  {
    file.sync_all()?; // otherwise the we'll need to read the length using seek
    let mut blen = file.metadata().context("Error reading metadata of file.")?.len();

    let mut lenbuf = [0u8; 8];
    delta.read_exact(&mut lenbuf).context("Error reading expected final length of file.")?;
    let len = u8aletou64(lenbuf);
    //println!("OP_LEN_B {:?}", len);
    // TODO: this condition should only be be true if file is file_a
    // Can we check this without wrapping ourselves in knots?
    if len < blen { // if the file should shrink, we must truncate it
        file.set_len(len).context("Failed to set length of file.")?;
        file.sync_all().context("Failed to sync file.")?; // otherwise the we'll need to read the length using seek
        blen = file.metadata().context("Error re-reading metadata of file.")?.len();
    };
    if len != blen { // if the file should have grown, it should have already grown due to OP_DIFFs
        // TODO: don't panic, find a way of returning an error.
        panic!("This delta expects file_b to be {:?} bytes long, not {:?} bytes.", len, blen);
    }
    Ok(())
}

fn op_hash_b(delta: &mut File, file_b: &mut File, blen: u64) -> Result<()> {
    let mut hashbuf = [0u8; 32];
    delta.read_exact(&mut hashbuf).context("Error reading expected final hash of file.")?;
    //println!("OP_HASH_B {:02X?}", hashbuf);
    let hash = hash_file(file_b, blen)?;
    if hash != hashbuf {
        panic!("This delta expects file_b's hash to be {:X?}, not {:X?}.", hashbuf, hash);
    };
    Ok(())
}

fn main() -> Result<()> {
    let args = Cli::from_args();

    // this only needs to be writeble if it is being updated in-place
    let mut file_a = OpenOptions::new().write(args.file_b.is_none())
                                       .read(true)
                                       .open(&args.file_a)
                                       .with_context(|| format!("Error opening {}", args.file_a))?;
    let alen = file_a.metadata().unwrap().len();

    let mut delta = File::open(args.delta_input)?;

    // this needs to be read/write, as its hash is checked after it is written
    let mut opt_file_b = match args.file_b {
        Some(file_b) => Some(
            OpenOptions::new().write(true)
                             .read(true)
                             .create_new(true)
                             .open(&file_b)
                             .with_context(|| format!("Error opening {}", file_b))?
        ),
        None => None
    };

    read_magic(&mut delta).context("Error reading magic (file identifier).")?;


    loop {
        let mut opbuf = [0u8; 1];
        let mut count_buf = [0u8; 8];
        delta.read_exact(&mut opbuf).context("Error readin opcode.")?;
        let opcode = opbuf[0];

        match opcode {
            OP_VER => {
                op_ver(&mut delta).context("Error reading version.")?;
            }
            OP_LEN_A => {
                op_len_a(&mut delta, alen).context("Error verifying length of file_a.")?;
            }
            OP_HASH_A => {
                op_hash_a(&mut delta, &mut file_a, alen).context("Error verifying hash of file_a.")?;
            }
            OP_SKIP => {
                delta.read_exact(&mut count_buf).context("Error reading count of bytes to skip.")?;
                let count = u8aletou64(count_buf);
                match opt_file_b {
                    Some(ref mut file_b) => {
                        //println!("OP_SKIP sparse_copy_data {:?}", count);
                        sparse_copy_data(file_b, &mut file_a, count).context("Error performing (potentially) sparse copy.")? // copy data from file_a
                    },
                    None => {
                        //println!("OP_SKIP {:?}", count);
                        file_a.seek(SeekFrom::Current(count as i64)).context("Error skipping bytes in file_a.")?; // skip, nothing to do
                    }
                }
            }
            OP_DIFF => {
                delta.read_exact(&mut count_buf)?;
                let count = u8aletou64(count_buf);
                match opt_file_b {
                    Some(ref mut file_b) => {
                        //println!("OP_DIFF copy_data {:?}", count);
                        file_a.seek(SeekFrom::Current(count as i64)).context("Error seeking past different bytes in file_a.")?; // skip data in file_a
                        copy_data(file_b, &mut delta, count).context("Error copying bytes from delta into file_b.")?; // copy data from delta
                    },
                    None => {
                        //println!("OP_DIFF {:?}", count);
                        copy_data(&mut file_a, &mut delta, count).context("Error copying data from delta into file_a")?; // copy data from delta
                    }
                }
            }
            OP_LEN_B => {
                let file = match opt_file_b {
                    Some(ref mut file_b) => file_b,
                    None => &mut file_a
                };
                op_len_b(&mut delta, file).context("Error verifying OP_LEN_B.")?;
            }
            OP_HASH_B => {
                match opt_file_b {
                    Some(ref mut file_b) => {
                        file_b.sync_all().context("Error syncing file_b")?; // otherwise the we'll need to read the length using seek
                        let blen = file_b.metadata().context("Error reading file_b metadata.")?.len();
                        //println!("blen: {:?}", blen);
                        op_hash_b(&mut delta, file_b, blen).context("Error verifying file_b hash.")?;
                    },
                    None => {
                        file_a.sync_all()?; // otherwise the we'll need to read the length using seek
                        let alen = file_a.metadata().context("Error reading file_a meatadata.")?.len();
                        //println!("blen: {:?}", alen);
                        op_hash_b(&mut delta, &mut file_a, alen).context("Error verifying hash of modified file_a.")?;
                    }
                }
            }
            OP_END => {
                //println!("OP_END");
                break;
            }
            _ => {
                panic!("error: bad format");
            }
        }
    }

	Result::Ok(())
}
