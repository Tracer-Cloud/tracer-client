// todo: check max size in kernels?
pub const ARGS_MAX_LEN: usize = 128;
pub const MAX_NUM_ARGS: usize = 5;

#[repr(C)]
#[derive(Debug)]
pub struct ProcessEnter {
    pub pid: i32,
    pub comm: [u8; 16], // todo: use TASK_COMM_LEN

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
        comm: [0; 16],
    };
}

#[cfg(feature = "user")]
pub fn from_bpf_str(s: &[u8]) -> anyhow::Result<&str> {
    let zero_pos = s.iter().position(|&x| x == 0);
    let s = match zero_pos {
        Some(pos) => &s[..pos],
        None => s,
    };
    Ok(std::str::from_utf8(s)?)
}

#[cfg(feature = "user")]
impl TryInto<tracer_common::trigger::Trigger> for &ProcessEnter {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<tracer_common::trigger::Trigger, Self::Error> {
        Ok(tracer_common::trigger::Trigger::Start {
            pid: self.pid as u32,
            file_name: from_bpf_str(self.file_name.as_slice())?.to_string(),
            comm: from_bpf_str(self.comm.as_slice())?.to_string(),
            argv: self
                .argv
                .iter()
                .take(self.len)
                .map(|x| from_bpf_str(x).unwrap().to_string())
                .collect(), // todo: improve
        })
    }
}
