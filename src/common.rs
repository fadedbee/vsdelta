pub const CHUNKSIZE: usize = 8;
pub const CHUNKLEN: u64 = CHUNKSIZE as u64;

pub const OP_CPY: u8 = 0xCC; // followed by count
pub const OP_IMM: u8 = 0x11; // followed by count, then by data[count]
pub const OP_END: u8 = 0xEE; // followed by:
			     // 8 bytes of alen
			     // 32 bytes of asha256
			     // 8 bytes of blen
			     // 32 bytes of bsha256
