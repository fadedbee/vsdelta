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

/* 
 * Appends "num" bytes at "offset" in src to dst.
 */
fn append_data(dst: &mut File, src: &mut File, num: u64, offset: u64) -> Result<()> {
    const COPY_CHUNKSIZE: usize = 8;
    const COPY_CHUNKLEN: u64 = COPY_CHUNKSIZE as u64;

    let num_chunks = num / COPY_CHUNKLEN;
    let remainder = num - num_chunks * COPY_CHUNKLEN;

    src.seek(SeekFrom::Current(-((num + offset) as i64)))?;

    let mut copybuf = vec![0u8; COPY_CHUNKSIZE];
    for _ in 0..(num / COPY_CHUNKLEN) {
        src.read(&mut copybuf)?;
        dst.write(&copybuf)?;
    }

    let mut copybuf = vec![0u8; remainder as usize];
    src.read(&mut copybuf)?;
    dst.write(&copybuf)?;

    src.seek(SeekFrom::Current(offset as i64))?;

    Ok(())
}

fn next_state(state: State, ochunk: &mut Vec<u8>, nchunk: &mut Vec<u8>, new: &mut File, delta: &mut File, offset: u64, chunklen: u64) -> Result<State> {
    Result::Ok(match state {
        State::Init => {
            if nchunk == ochunk {
                delta.write(&[COPY])?;
                State::Matching(chunklen)
            } else {
                delta.write(&[IMMEDIATE])?;
                State::Different(chunklen)
            }
        },
        State::Matching(num) => {
            if nchunk == ochunk {
                println!("0same: {:02X?} {:02X?}", ochunk, nchunk);
                State::Matching(num + chunklen)
            } else {
                println!("0diff: {:02X?} {:02X?}", ochunk, nchunk);
                delta.write(&u64tou8ale(num))?;
                delta.write(&[IMMEDIATE])?;
                State::Different(chunklen)
            }
        },
        State::Different(num) => {
            if nchunk == ochunk {
                println!("1same: {:02X?} {:02X?}", ochunk, nchunk);
                delta.write(&u64tou8ale(num))?;
                
                // append data from new to delta
                append_data(delta, new, num, chunklen)?;

                delta.write(&[COPY])?;
                State::Matching(chunklen)
            } else {
                println!("1diff: {:02X?} {:02X?}", ochunk, nchunk);
                State::Different(num + chunklen)
            }
        }
    })
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

    let mut ochunk = vec![0u8; CHUNKSIZE];
    let mut nchunk = vec![0u8; CHUNKSIZE];
    let num_chunks = min_len / CHUNKLEN;
    let remainder = min_len - num_chunks * CHUNKLEN;
    println!("remainder: {:?}", remainder);

    // process all of the whole chunks
    for cnum in 0..num_chunks {
        old.read(&mut ochunk)?;
        new.read(&mut nchunk)?;
        state = next_state(state, &mut ochunk, &mut nchunk, &mut new, &mut delta, cnum * CHUNKLEN, CHUNKLEN)?;
    }

    // process the final, partial chunk.
    let mut partial_ochunk = vec![0u8; remainder as usize];
    let mut partial_nchunk = vec![0u8; remainder as usize];
    old.read(&mut partial_ochunk)?;
    new.read(&mut partial_nchunk)?;
    state = next_state(state, &mut partial_ochunk, &mut partial_nchunk, &mut new, &mut delta, num_chunks * CHUNKLEN, remainder)?;

    // TODO: deal with the remaining bytes of *either* "old" or "new"

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

            // append data from new to delta
            append_data(&mut delta, &mut new, num, 0)?;
        }
    }

	Result::Ok(())
}
