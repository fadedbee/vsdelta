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

const BUFSIZE: usize = 8;
const BUFLEN: u64 = BUFSIZE as u64;

const COPY: u8 = 0xCC; // followed by count
const IMMEDIATE: u8 = 0x11; // followed by count, then by data[count]

enum State {
    Init,
    Matching{num: u64, count_pos: u64},
    Different{num: u64, count_pos: u64}
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

    let mut obuf = [0; BUFSIZE];
    let mut nbuf = [0; BUFSIZE];
    let num_bufs = min_len / BUFLEN;
    for _ in 0..num_bufs {
        old.read(&mut obuf)?;
        new.read(&mut nbuf)?;
        state = match state {
            State::Init => {
                if nbuf == obuf {
                    delta.write(&[COPY])?;
                    delta.write(&u64tou8ale(0))?;
                    State::Matching{num: BUFLEN, count_pos: 1}
                } else {
                    delta.write(&[IMMEDIATE])?;
                    delta.write(&u64tou8ale(0))?;
                    State::Different{num: BUFLEN, count_pos: 1}
                }
            },
            State::Matching{num, count_pos} => {
                if nbuf == obuf {
                    State::Matching{num: num + BUFLEN, count_pos}
                } else {
                    delta.seek(SeekFrom::Start(count_pos))?;
                    delta.write(&u64tou8ale(num))?;
                    delta.seek(SeekFrom::End(0))?;
                    delta.write(&[IMMEDIATE])?;
                    let count_pos = delta.seek(SeekFrom::End(0))?;
                    delta.write(&u64tou8ale(0))?;
                    delta.write(&nbuf)?;
                    State::Different{num: BUFLEN, count_pos}
                }
            },
            State::Different{num, count_pos} => {
                if nbuf == obuf {
                    delta.seek(SeekFrom::Start(count_pos))?;
                    delta.write(&u64tou8ale(num))?;
                    delta.seek(SeekFrom::End(0))?;
                    delta.write(&[COPY])?;
                    let count_pos = delta.seek(SeekFrom::End(0))?;
                    delta.write(&u64tou8ale(0))?;
                    State::Matching{num: BUFLEN, count_pos}
                } else {
                    delta.write(&nbuf)?;
                    State::Different{num: num + BUFLEN, count_pos}
                }
            }
        }
    }

    // write final count
    match state {
        State::Init => {
            // files were empty
        },
        State::Matching{num, count_pos} => {
            delta.seek(SeekFrom::Start(count_pos))?;
            delta.write(&u64tou8ale(num))?;
        },
        State::Different{num, count_pos} => {
            delta.seek(SeekFrom::Start(count_pos))?;
            delta.write(&u64tou8ale(num))?;
        }
    }

    println!("Hello, world!");

	Result::Ok(())
}
