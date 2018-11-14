pub const O_DIRECT: i32 = 0o0040000;
pub const VERSION_LEN: usize = 4;
pub const VERSION: &[u8; VERSION_LEN] = &[0xA0, 0xF1, 0xB2, 0x01];
pub const BUFFER_SIZE: usize = 64 * 1024;
pub const MEM_MAP_THRESHOLD: usize = 64 * 1024; // 64k
