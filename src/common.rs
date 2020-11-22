pub const CHUNKSIZE: usize = 8;
pub const CHUNKLEN: u64 = CHUNKSIZE as u64;


pub const OP_VER: u8 = 0x00;  // followed by X.Y.Z bytes
pub const OP_HASH_A: u8 = 0x4A;
pub const OP_HASH_B: u8 = 0x4B;
pub const OP_LEN_A: u8 = 0x7A;
pub const OP_LEN_B: u8 = 0x7B;
pub const OP_SKIP: u8 = 0x55; // followed by count
pub const OP_ADD: u8 = 0xAA;  // followed by count, then by data[count]
pub const OP_HOLE: u8 = 0xAA; // followed by count
pub const OP_END: u8 = 0xEE;  
