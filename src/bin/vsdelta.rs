use structopt::StructOpt;
use std::fs::File;
use std::io::{SeekFrom, Result};
use std::cmp::min;
use std::io::prelude::*;
use vsdelta::common::{CHUNKSIZE, CHUNKLEN, OP_CPY, OP_IMM, OP_END};

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

#[derive(Debug)]
enum State {
    Init,
    Matching(u64),
    Different(u64)
}

/* 
 * Appends "num" bytes at "offset" in src to dst.
 */
fn append_data(dst: &mut File, src: &mut File, num: u64, offset: u64) -> Result<()> {
    const OP_CPY_CHUNKSIZE: usize = 8;
    const OP_CPY_CHUNKLEN: u64 = OP_CPY_CHUNKSIZE as u64;

    let num_chunks = num / OP_CPY_CHUNKLEN;
    let remainder = num - num_chunks * OP_CPY_CHUNKLEN;

    src.seek(SeekFrom::Current(-((num + offset) as i64)))?;

    let mut copybuf = vec![0u8; OP_CPY_CHUNKSIZE];
    for _ in 0..(num / OP_CPY_CHUNKLEN) {
        src.read(&mut copybuf)?;
        dst.write(&copybuf)?;
    }

    let mut copybuf = vec![0u8; remainder as usize];
    src.read(&mut copybuf)?;
    dst.write(&copybuf)?;

    src.seek(SeekFrom::Current(offset as i64))?;

    Ok(())
}

fn next_state(state: State, ochunk: &mut Vec<u8>, nchunk: &mut Vec<u8>, new: &mut File, delta: &mut File, chunklen: u64) -> Result<State> {
    Result::Ok(match state {
        State::Init => {
            if nchunk == ochunk {
                delta.write(&[OP_CPY])?;
                State::Matching(chunklen)
            } else {
                delta.write(&[OP_IMM])?;
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
                delta.write(&[OP_IMM])?;
                State::Different(chunklen)
            }
        },
        State::Different(num) => {
            if nchunk == ochunk {
                println!("1same: {:02X?} {:02X?}", ochunk, nchunk);
                delta.write(&u64tou8ale(num))?;
                
                // append data from new to delta
                append_data(delta, new, num, chunklen)?;

                delta.write(&[OP_CPY])?;
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

    println!("old: {:?}, new: {:?}", old.seek(SeekFrom::Current(0))?, new.seek(SeekFrom::Current(0))?);
    println!("0state: {:?}", state);
    // process all of the whole chunks
    for _ in 0..num_chunks {
        old.read(&mut ochunk)?;
        new.read(&mut nchunk)?;
        println!("old: {:?}, new: {:?}", old.seek(SeekFrom::Current(0))?, new.seek(SeekFrom::Current(0))?);
        println!("1state: {:?}", state);
        state = next_state(state, &mut ochunk, &mut nchunk, &mut new, &mut delta, CHUNKLEN)?;
    }

    println!("old: {:?}, new: {:?}", old.seek(SeekFrom::Current(0))?, new.seek(SeekFrom::Current(0))?);
    println!("2state: {:?}", state);
    // process the final, partial chunk.
    let remainder = min_len - num_chunks * CHUNKLEN;
    println!("remainder: {:?}", remainder);
    let mut partial_ochunk = vec![0u8; remainder as usize];
    let mut partial_nchunk = vec![0u8; remainder as usize];
    old.read(&mut partial_ochunk)?;
    new.read(&mut partial_nchunk)?;
    state = next_state(state, &mut partial_ochunk, &mut partial_nchunk, &mut new, &mut delta, remainder)?;

    if nlen > min_len {
        // new file is longer - we must copy the excess
        let excess = nlen - min_len;
        println!("excess: {:?}", excess);

        println!("old: {:?}, new: {:?}", old.seek(SeekFrom::Current(0))?, new.seek(SeekFrom::Current(0))?);
        println!("3state: {:?}", state);
        state = match state {
            State::Init => { // the old file was empty
                delta.write(&[OP_IMM])?;
                State::Different(excess)
            },
            State::Matching(num) => { // the new file matched the end of the old file
                delta.write(&u64tou8ale(num))?;
                delta.write(&[OP_IMM])?;
                State::Different(excess)
            },
            State::Different(num) => { // the new file is already different to the end of the old file 
                State::Different(num + excess)
            }
        };

        // update the seek position of new, as we haven't read from it for a comparison
        println!("old: {:?}, new: {:?}", old.seek(SeekFrom::Current(0))?, new.seek(SeekFrom::Current(0))?);
        new.seek(SeekFrom::Current(excess as i64))?;
        println!("old: {:?}, new: {:?}", old.seek(SeekFrom::Current(0))?, new.seek(SeekFrom::Current(0))?);
    }

    // write final count
    println!("old: {:?}, new: {:?}", old.seek(SeekFrom::Current(0))?, new.seek(SeekFrom::Current(0))?);
    println!("4state: {:?}", state);
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

    // write end
    delta.write(&[OP_END])?;

	Result::Ok(())
}
