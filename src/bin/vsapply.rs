use structopt::StructOpt;
use std::fs::{File, OpenOptions};
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
    const OP_SKIP_CHUNKSIZE: usize = 59;
    const OP_SKIP_CHUNKLEN: u64 = OP_SKIP_CHUNKSIZE as u64;

    let num_chunks = num / OP_SKIP_CHUNKLEN;
    let remainder = num - num_chunks * OP_SKIP_CHUNKLEN;

    let mut copybuf = vec![0xFFu8; OP_SKIP_CHUNKSIZE];
    for _ in 0..(num / OP_SKIP_CHUNKLEN) {
        let pos = src.seek(SeekFrom::Current(0)).unwrap();
        println!("pos: {:?}", pos);
        src.read_exact(&mut copybuf).unwrap();
        println!("copybuf {:X?}", copybuf);
        dst.write(&copybuf).unwrap();
    }

    let mut copybuf = vec![0u8; remainder as usize];
    src.read_exact(&mut copybuf).unwrap();
    println!("copybuf {:X?}", copybuf);
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
    println!("OP_VER {:?}.{:?}.{:?}", ver[0], ver[1], ver[2]);
    Ok(())
}

fn op_len_a(delta: &mut File, alen: u64)-> Result<()>  {
    let mut lenbuf = [0u8; 8];
    delta.read_exact(&mut lenbuf)?;
    let len = u8aletou64(lenbuf);
    println!("OP_LEN_A {:?}", len);
    if len != alen {
        panic!("This delta expects file_a to be {:?} bytes long, not {:?} bytes.", len, alen);
    }
    Ok(())
}

fn op_hash_a(delta: &mut File, file_a: &mut File, alen: u64) -> Result<()> {
    let mut hashbuf = [0u8; 32];
    delta.read_exact(&mut hashbuf)?;
    println!("OP_HASH_A {:02X?}", hashbuf);
    let hash = hash_file(file_a, alen)?;
    if hash != hashbuf {
        panic!("This delta expects file_a's hash to be {:X?}, not {:X?}.", hashbuf, hash);
    };
    Ok(())
}

fn op_len_b(delta: &mut File, blen: u64)-> Result<()>  {
    let mut lenbuf = [0u8; 8];
    delta.read_exact(&mut lenbuf)?;
    let len = u8aletou64(lenbuf);
    println!("OP_LEN_B {:?}", len);
    if len != blen {
        panic!("This delta expects file_b to be {:?} bytes long, not {:?} bytes.", len, blen);
    }
    Ok(())
}

fn op_hash_b(delta: &mut File, file_b: &mut File, blen: u64) -> Result<()> {
    let mut hashbuf = [0u8; 32];
    delta.read_exact(&mut hashbuf)?;
    println!("OP_HASH_B {:02X?}", hashbuf);
    /*
    let hash = hash_file(file_b, blen)?;
    if hash != hashbuf {
        panic!("This delta expects file_b's hash to be {:X?}, not {:X?}.", hashbuf, hash);
    };
    */
    Ok(())
}

//https://github.com/fadedbee/vsdelta/issues/7
//$ reset ; cp  ../virtsync/test-dir/target.txt ./ && ls -lA target.txt && b3sum target.txt && cargo run --bin vsapply target.txt ../virtsync/test-dir/target.txt.2020-12-0* && ls target.txt && b3sum target.txt
//thread 'main' panicked at 'This delta expects file_b to be 43999 bytes long, not 43964 bytes.', src/bin/vsapply.rs:107:9

fn main() -> Result<()> {
    let args = Cli::from_args();

    // this only needs to be writeble if it is being updated in-place
    let mut file_a = OpenOptions::new().write(args.file_b.is_none())
                                       .read(true)
                                       .open(args.file_a).unwrap();
    let alen = file_a.metadata().unwrap().len();

    let mut delta = File::open(args.delta_input)?;

    // this needs to be read/write, as its hash is checked after it is written
    let mut opt_file_b = match args.file_b {
        Some(file_b) => Some(
            OpenOptions::new().write(true)
                             .read(true)
                             .create_new(true)
                             .open(file_b).unwrap()
        ),
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
            OP_HASH_A => {
                op_hash_a(&mut delta, &mut file_a, alen)?;
            }
            OP_SKIP => {
                delta.read_exact(&mut count_buf)?;
                let count = u8aletou64(count_buf);
                println!("OP_SKIP {:?}", count);
                match opt_file_b {
                    Some(ref mut file_b) => {
                        println!("OP_SKIP copy_data {:?}", count);
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
            OP_LEN_B => {
                let blen = match opt_file_b {
                    Some(ref mut file_b) => {
                        file_b.sync_all()?; // otherwise the we'll need to read the length using seek
                        file_b.metadata().unwrap().len()
                    },
                    None => {
                        file_a.sync_all()?; // otherwise the we'll need to read the length using seek
                        file_a.metadata().unwrap().len()
                    }
                };
                op_len_b(&mut delta, blen)?;
            }
            OP_HASH_B => {
                match opt_file_b {
                    Some(ref mut file_b) => {
                        file_b.sync_all()?; // otherwise the we'll need to read the length using seek
                        let blen = file_b.metadata().unwrap().len();
                        println!("blen: {:?}", blen);
                        op_hash_b(&mut delta, file_b, blen)?;
                    },
                    None => {
                        file_a.sync_all()?; // otherwise the we'll need to read the length using seek
                        let alen = file_a.metadata().unwrap().len();
                        println!("blen: {:?}", alen);
                        op_hash_b(&mut delta, &mut file_a, alen)?;
                    }
                }
            }
            OP_END => {
                println!("OP_END");
                break;
            }
            _ => {
                panic!("error: bad format");
            }
        }
    }

	Result::Ok(())
}
