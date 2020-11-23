use structopt::StructOpt;
use std::fs::File;
use std::io::{SeekFrom, Result};
use std::cmp::min;
use std::io::prelude::*;
use vsdelta::common::*;

#[derive(StructOpt)]
struct Cli {
    file_a: String,
    file_b: String,
    delta_output: String,
}

pub mod built_info {
    // The file has been placed there by the build script.
    include!(concat!(env!("OUT_DIR"), "/built.rs"));
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

fn write_magic(delta: &mut File) -> Result<()> {
    delta.write("vsdelta".as_bytes())?;
    Ok(())
}

fn write_op_ver(delta: &mut File, version: [u8; 3]) -> Result<()> {
    delta.write(&[OP_VER])?;
    delta.write(&version)?;
    Ok(())
}

fn write_op_len_a(delta: &mut File, alen: u64) -> Result<()> {
    delta.write(&[OP_LEN_A])?;
    delta.write(&u64tou8ale(alen))?;
    Ok(())
}

fn write_op_sha256_a(delta: &mut File, file_a: &mut File, alen: u64) -> Result<()> {
    file_a.seek(SeekFrom::Start(0))?; // rewind
    let hash_a = sha256(file_a, alen)?;
    file_a.seek(SeekFrom::Start(0))?; // rewind
    
    delta.write(&[OP_SHA256_A])?;
    delta.write(&hash_a)?;

    Ok(())
}

fn write_op_sha256_b(delta: &mut File, file_b: &mut File, blen: u64) -> Result<()> {
    file_b.seek(SeekFrom::Start(0))?; // rewind
    let hash_a = sha256(file_b, blen)?;
    file_b.seek(SeekFrom::Start(0))?; // rewind
    
    delta.write(&[OP_SHA256_B])?;
    delta.write(&hash_a)?;

    Ok(())
}

fn write_op_len_b(delta: &mut File, blen: u64) -> Result<()> {
    delta.write(&[OP_LEN_B])?;
    delta.write(&u64tou8ale(blen))?;
    Ok(())
}

fn write_op_end(delta: &mut File) -> Result<()> {
    delta.write(&[OP_END])?;
    Ok(())
}

/* 
 * Appends "num" bytes at "offset" in src to dst.
 */
fn append_data(dst: &mut File, src: &mut File, num: u64, offset: u64) -> Result<()> {

    let num_chunks = num / BIGCHUNKLEN;
    let remainder = num - num_chunks * BIGCHUNKLEN;

    src.seek(SeekFrom::Current(-((num + offset) as i64)))?;

    let mut copybuf = vec![0u8; BIGCHUNKSIZE];
    for _ in 0..(num / BIGCHUNKLEN) {
        src.read(&mut copybuf)?;
        dst.write(&copybuf)?;
    }

    let mut copybuf = vec![0u8; remainder as usize];
    src.read(&mut copybuf)?;
    dst.write(&copybuf)?;

    src.seek(SeekFrom::Current(offset as i64))?;

    Ok(())
}

fn next_state(state: State, achunk: &mut Vec<u8>, bchunk: &mut Vec<u8>, file_b: &mut File, delta: &mut File, chunklen: u64) -> Result<State> {
    Result::Ok(match state {
        State::Init => {
            if bchunk == achunk {
                delta.write(&[OP_SKIP])?;
                State::Matching(chunklen)
            } else {
                delta.write(&[OP_DIFF])?;
                State::Different(chunklen)
            }
        },
        State::Matching(num) => {
            if bchunk == achunk {
                println!("0same: {:02X?} {:02X?}", achunk, bchunk);
                State::Matching(num + chunklen)
            } else {
                println!("0diff: {:02X?} {:02X?}", achunk, bchunk);
                delta.write(&u64tou8ale(num))?;
                delta.write(&[OP_DIFF])?;
                State::Different(chunklen)
            }
        },
        State::Different(num) => {
            if bchunk == achunk {
                println!("1same: {:02X?} {:02X?}", achunk, bchunk);
                delta.write(&u64tou8ale(num))?;
                
                // append data from file_b to delta
                append_data(delta, file_b, num, chunklen)?;

                delta.write(&[OP_SKIP])?;
                State::Matching(chunklen)
            } else {
                println!("1diff: {:02X?} {:02X?}", achunk, bchunk);
                State::Different(num + chunklen)
            }
        }
    })
}

fn main() -> Result<()> {
	let args = Cli::from_args();

    let mut file_a = File::open(args.file_a)?;
    let alen = file_a.metadata().unwrap().len();
    let mut file_b = File::open(args.file_b)?;
    let blen = file_b.metadata().unwrap().len();
    let mut delta = File::create(args.delta_output)?;

    let min_len = min(alen, blen);

    let mut state = State::Init;

    let mut achunk = vec![0u8; CHUNKSIZE];
    let mut bchunk = vec![0u8; CHUNKSIZE];
    let num_chunks = min_len / CHUNKLEN;

    write_magic(&mut delta)?;
    write_op_ver(&mut delta, [
        built_info::PKG_VERSION_MAJOR.parse::<u8>().unwrap(), 
        built_info::PKG_VERSION_MINOR.parse::<u8>().unwrap(), 
        built_info::PKG_VERSION_PATCH.parse::<u8>().unwrap()])?;
    write_op_len_a(&mut delta, alen)?;
    write_op_sha256_a(&mut delta, &mut file_a, alen)?;

    println!("file_a: {:?}, file_b: {:?}", file_a.seek(SeekFrom::Current(0))?, file_b.seek(SeekFrom::Current(0))?);
    println!("0state: {:?}", state);
    // process all of the whole chunks
    for _ in 0..num_chunks {
        file_a.read(&mut achunk)?;
        file_b.read(&mut bchunk)?;
        println!("file_a: {:?}, file_b: {:?}", file_a.seek(SeekFrom::Current(0))?, file_b.seek(SeekFrom::Current(0))?);
        println!("1state: {:?}", state);
        state = next_state(state, &mut achunk, &mut bchunk, &mut file_b, &mut delta, CHUNKLEN)?;
    }

    println!("file_a: {:?}, file_b: {:?}", file_a.seek(SeekFrom::Current(0))?, file_b.seek(SeekFrom::Current(0))?);
    println!("2state: {:?}", state);
    // process the final, partial chunk.
    let remainder = min_len - num_chunks * CHUNKLEN;
    println!("remainder: {:?}", remainder);
    let mut partial_achunk = vec![0u8; remainder as usize];
    let mut partial_bchunk = vec![0u8; remainder as usize];
    file_a.read(&mut partial_achunk)?;
    file_b.read(&mut partial_bchunk)?;
    state = next_state(state, &mut partial_achunk, &mut partial_bchunk, &mut file_b, &mut delta, remainder)?;

    if blen > min_len {
        // file_b file is longer - we must copy the excess
        let excess = blen - min_len;
        println!("excess: {:?}", excess);

        println!("file_a: {:?}, file_b: {:?}", file_a.seek(SeekFrom::Current(0))?, file_b.seek(SeekFrom::Current(0))?);
        println!("3state: {:?}", state);
        state = match state {
            State::Init => { // the file_a file was empty
                delta.write(&[OP_DIFF])?;
                State::Different(excess)
            },
            State::Matching(num) => { // the file_b file matched the end of the file_a file
                delta.write(&u64tou8ale(num))?;
                delta.write(&[OP_DIFF])?;
                State::Different(excess)
            },
            State::Different(num) => { // the file_b file is already different to the end of the file_a file 
                State::Different(num + excess)
            }
        };

        // update the seek position of file_b, as we haven't read from it for a comparison
        println!("file_a: {:?}, file_b: {:?}", file_a.seek(SeekFrom::Current(0))?, file_b.seek(SeekFrom::Current(0))?);
        file_b.seek(SeekFrom::Current(excess as i64))?;
        println!("file_a: {:?}, file_b: {:?}", file_a.seek(SeekFrom::Current(0))?, file_b.seek(SeekFrom::Current(0))?);
    }

    // write final count
    println!("file_a: {:?}, file_b: {:?}", file_a.seek(SeekFrom::Current(0))?, file_b.seek(SeekFrom::Current(0))?);
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

            // append data from file_b to delta
            append_data(&mut delta, &mut file_b, num, 0)?;
        }
    }

    // write end
    write_op_len_b(&mut delta, blen)?; // FIXME: calculate sha256(b) as we read file_b, to save I/O
    write_op_sha256_b(&mut delta, &mut file_b, blen)?; // FIXME: calculate sha256(b) as we read file_b, to save I/O
    write_op_end(&mut delta)?;

	Result::Ok(())
}
