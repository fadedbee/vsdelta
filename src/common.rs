use sha2::{Sha256, Digest};
use std::fs::File;
use std::io::Result;
use std::io::prelude::*;

pub const CHUNKSIZE: usize = 8;
pub const CHUNKLEN: u64 = CHUNKSIZE as u64;
pub const BIGCHUNKSIZE: usize = 8;
pub const BIGCHUNKLEN: u64 = BIGCHUNKSIZE as u64;

pub const OP_VER: u8 = 0x00;      // followed by X.Y.Z bytes
pub const OP_LEN_A: u8 = 0x77;    // followed by length
pub const OP_SHA256_A: u8 = 0xAA; // followed by 32 bytes of hash

pub const OP_SKIP: u8 = 0x55;     // followed by count
pub const OP_DIFF: u8 = 0xDD;     // followed by count, then by data[count]
pub const OP_HOLE: u8 = 0x44;     // followed by count

pub const OP_LEN_B: u8 = 0x88;    // followed by length
pub const OP_SHA256_B: u8 = 0xBB; // followed by 32 bytes of hash
pub const OP_END: u8 = 0xEE;

/* computes the sha256sum of the file */
pub fn sha256(file: &mut File, file_len: u64) -> Result<[u8; 32]> {
	let mut hasher = Sha256::new();

	let mut buf = [0u8; BIGCHUNKSIZE];
	let num_chunks = file_len / BIGCHUNKLEN;
	for _ in 0..num_chunks {
		file.read_exact(&mut buf)?;		
		hasher.update(buf);
	}

	let remainder = file_len as usize - num_chunks as usize * BIGCHUNKSIZE;
	let mut buf = vec![0u8; remainder];
	file.read_exact(&mut buf)?;		
	hasher.update(buf);

	let hash = hasher.finalize();
	Ok(*hash.as_ref())
}