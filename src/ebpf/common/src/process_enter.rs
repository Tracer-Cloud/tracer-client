// todo: check max size in kernels?
pub const ARGS_MAX_LEN: usize = 128;
pub const MAX_NUM_ARGS: usize = 5;

#[repr(C)]
#[derive(Debug)]
pub struct ProcessEnter {
    pub pid: i32,
    pub file_name: [u8; 32],
    // pub argv: [u8; ARGS_MAX_LEN],
    pub argv: [[u8; ARGS_MAX_LEN]; MAX_NUM_ARGS],
    pub len: usize,
}

impl ProcessEnter {
    pub const EMPTY: Self = Self {
        pid: 0,
        file_name: [0; 32],
        argv: [[0; ARGS_MAX_LEN]; MAX_NUM_ARGS],
        len: 0,
    };
}
