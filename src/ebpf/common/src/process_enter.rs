// todo: check max size in kernels?
pub const ARGS_MAX_LEN: usize = 128;
pub const MAX_NUM_ARGS: usize = 5;

#[repr(C)]
#[derive(Debug)]
pub enum ProcessEnterType {
    Start,
    Finish,
}

#[repr(C)]
#[derive(Debug)]
pub struct ProcessRawTrigger {
    pub pid: i32,
    pub ppid: i32,
    pub event_type: ProcessEnterType,

    pub comm: [u8; 16], // todo: use TASK_COMM_LEN

    pub file_name: [u8; 32],
    // pub argv: [u8; ARGS_MAX_LEN],
    pub argv: [[u8; ARGS_MAX_LEN]; MAX_NUM_ARGS],
    pub len: usize,

    pub time: u64,
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
use tracer_common::trigger::*;

#[cfg(feature = "user")]
impl TryInto<tracer_common::trigger::Trigger> for &ProcessRawTrigger {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<tracer_common::trigger::Trigger, Self::Error> {
        Ok(match self.event_type {
            ProcessEnterType::Start => {
                tracer_common::trigger::Trigger::Start(ProcessTrigger {
                    pid: self.pid as usize,
                    ppid: self.ppid as usize,
                    file_name: from_bpf_str(self.file_name.as_slice())?.to_string(),
                    comm: from_bpf_str(self.comm.as_slice())?.to_string(),
                    argv: self
                        .argv
                        .iter()
                        .take(self.len)
                        .map(|x| from_bpf_str(x).unwrap().to_string())
                        .collect(), // todo: improve
                    start_time: self.time,
                })
            }
            ProcessEnterType::Finish => tracer_common::trigger::Trigger::Finish(FinishTrigger {
                pid: self.pid as usize,
            }),
        })
    }
}
