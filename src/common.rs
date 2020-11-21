pub const CHUNKSIZE: usize = 8;
pub const CHUNKLEN: u64 = CHUNKSIZE as u64;

pub const COPY: u8 = 0xCC; // followed by count
pub const IMMEDIATE: u8 = 0x11; // followed by count, then by data[count]
