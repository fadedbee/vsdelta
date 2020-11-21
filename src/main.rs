use structopt::StructOpt;
use std::fs::File;
use std::io::{SeekFrom, Result};
use std::cmp::min;
use std::io::prelude::*;

#[derive(StructOpt)]
struct Cli {
    old_file: String,
    new_file: String,
    delta_file: String,
}

// little endian
#[inline]
fn u64tou8ale(v: u64) -> [u8; 8] {
    [
        v as u8,
        (v >> 8) as u8,
        (v >> 16) as u8,
        (v >> 24) as u8,
        (v >> 32) as u8,
        (v >> 40) as u8,
        (v >> 48) as u8,
        (v >> 56) as u8,
    ]
}

const CHUNKSIZE: usize = 8;
const CHUNKLEN: u64 = CHUNKSIZE as u64;

const COPY: u8 = 0xCC; // followed by count
const IMMEDIATE: u8 = 0x11; // followed by count, then by data[count]

enum State {
    Init,
    Matching(u64),
    Different(u64)
}

fn main() -> Result<()> {
	let args = Cli::from_args();

    let mut old = File::open(args.old_file)?;
    let olen = old.metadata().unwrap().len();
    let mut new = File::open(args.new_file)?;
    let nlen = new.metadata().unwrap().len();
    let mut delta = File::create(args.delta_file)?;

    let min_len = min(olen, nlen);

    let mut state = State::Init;

    let mut ochunk = [0; CHUNKSIZE];
    let mut nchunk = [0; CHUNKSIZE];
    let mut copybuf = [0; CHUNKSIZE];
    let num_chunks = min_len / CHUNKLEN;
    for cnum in 0..num_chunks {
        old.read(&mut ochunk)?;
        new.read(&mut nchunk)?;
        state = match state {
            State::Init => {
                if nchunk == ochunk {
                    delta.write(&[COPY])?;
                    State::Matching(CHUNKLEN)
                } else {
                    delta.write(&[IMMEDIATE])?;
                    State::Different(CHUNKLEN)
                }
            },
            State::Matching(num) => {
                if nchunk == ochunk {
                    println!("0same: {:02X?} {:02X?}", ochunk, nchunk);
                    State::Matching(num + CHUNKLEN)
                } else {
                    println!("0diff: {:02X?} {:02X?}", ochunk, nchunk);
                    delta.write(&u64tou8ale(num))?;
                    delta.write(&[IMMEDIATE])?;
                    State::Different(CHUNKLEN)
                }
            },
            State::Different(num) => {
                if nchunk == ochunk {
                    println!("1same: {:02X?} {:02X?}", ochunk, nchunk);
                    delta.write(&u64tou8ale(num))?;
                    
                    // copy data
                    new.seek(SeekFrom::Start(cnum * CHUNKLEN - num))?;
                    for _ in 0..(num / CHUNKLEN) {
                        new.read(&mut copybuf)?;
                        delta.write(&copybuf)?;
                    }
                    new.seek(SeekFrom::Start((cnum + 1) * CHUNKLEN))?; // restore "new" seek position

                    delta.write(&[COPY])?;
                    State::Matching(CHUNKLEN)
                } else {
                    println!("1diff: {:02X?} {:02X?}", ochunk, nchunk);
                    State::Different(num + CHUNKLEN)
                }
            }
        }
    }

    // write final count
    match state {
        State::Init => {
            // files were empty
        },
        State::Matching(num) => {
            delta.write(&u64tou8ale(num))?;
        },
        State::Different(num) => {
            delta.write(&u64tou8ale(num))?;

            // copy data
            new.seek(SeekFrom::Start(num_chunks * CHUNKLEN - num))?;
            for _ in 0..(num / CHUNKLEN) {
                new.read(&mut copybuf)?;
                delta.write(&copybuf)?;
            }
            new.seek(SeekFrom::Start((num_chunks + 1) * CHUNKLEN))?; // restore "new" seek position

        }
    }

    println!("Hello, world2!");

	Result::Ok(())
}
