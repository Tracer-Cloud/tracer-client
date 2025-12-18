use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use shlex;
use std::fmt;

fn join_args(argv: &[String]) -> String {
    shlex::try_join(argv.iter().map(|s| s.as_str())).unwrap_or_else(|_| argv.join(" "))
}

fn split_args(cmd: &str) -> Vec<String> {
    shlex::split(cmd).unwrap_or_else(|| cmd.split_whitespace().map(String::from).collect())
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct ProcessStartTrigger {
    /// Process ID
    pub pid: usize,
    /// Parent process ID
    pub ppid: usize,
    /// Command name (without path)
    pub comm: String,
    /// Command arguments (the first element is the command)
    pub argv: Vec<String>,
    /// Command string (from concatenating argv)
    pub command_string: String,
    /// Command start time
    pub started_at: DateTime<Utc>,
}

fn unquote(mut argv: Vec<String>) -> Vec<String> {
    for arg in argv.iter_mut() {
        if let Ok(unquoted) = enquote::unquote(arg) {
            *arg = unquoted;
        }
    }
    argv
}

impl ProcessStartTrigger {
    pub fn from_bpf_event(
        pid: u32,
        ppid: u32,
        comm: &str,
        argv: Vec<String>,
        timestamp_ns: u64,
    ) -> Self {
        const NS_PER_SEC: u64 = 1_000_000_000;
        Self {
            pid: pid as usize,
            ppid: ppid as usize,
            comm: comm.to_string(),
            command_string: join_args(&argv),
            argv: unquote(argv),
            started_at: DateTime::from_timestamp(
                (timestamp_ns / NS_PER_SEC) as i64,
                (timestamp_ns % NS_PER_SEC) as u32,
            )
            .unwrap(),
        }
    }

    pub fn from_name_and_args<A: AsRef<str>>(
        pid: usize,
        ppid: usize,
        name: &str,
        args: &[A],
    ) -> Self {
        let argv: Vec<String> = args.iter().map(|s| s.as_ref().to_string()).collect();
        Self {
            pid,
            ppid,
            comm: name.to_string(),
            command_string: join_args(&argv),
            argv: unquote(argv),
            started_at: Utc::now(),
        }
    }

    pub fn from_command_string(pid: usize, ppid: usize, command_string: &str) -> Self {
        let argv = split_args(command_string);
        let comm = argv.first().cloned().unwrap_or_default();
        Self {
            pid,
            ppid,
            comm,
            argv: unquote(argv),
            command_string: command_string.to_string(),
            started_at: Utc::now(),
        }
    }
}

/// A trigger indicating a process exited. `exit_reason` is only set if known,
/// e.g., via OOM tracking or future extensions.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct ProcessEndTrigger {
    pub pid: usize,
    pub finished_at: DateTime<Utc>,
    pub exit_reason: Option<ExitReason>,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct FileOpenTrigger {
    pub pid: u32,                 // pid of the process that opened the file
    pub filename: String,         // it can be the full path or just the name
    pub size_bytes: i128, // -1 if we can't get the size of the file, otherwise the size in bytes
    pub timestamp: DateTime<Utc>, // timestamp of the event
    pub file_full_path: String, // we use it to understand if 2 equals filenames are the same file
}

#[derive(Debug, Clone)]
pub struct OutOfMemoryTrigger {
    pub pid: usize,
    pub upid: u64,
    pub comm: String,
    pub timestamp: DateTime<Utc>,
}

/// Format duration in human-readable form
pub fn format_duration_ns(duration_ns: u64) -> String {
    if duration_ns == 0 {
        "N/A".to_string()
    } else if duration_ns < 1_000 {
        format!("{}ns", duration_ns)
    } else if duration_ns < 1_000_000 {
        format!("{:.2}µs", duration_ns as f64 / 1_000.0)
    } else if duration_ns < 1_000_000_000 {
        format!("{:.2}ms", duration_ns as f64 / 1_000_000.0)
    } else {
        format!("{:.3}s", duration_ns as f64 / 1_000_000_000.0)
    }
}

#[derive(Debug, Clone)]
pub enum Trigger {
    ProcessStart(ProcessStartTrigger),
    ProcessEnd(ProcessEndTrigger),
    OutOfMemory(OutOfMemoryTrigger),
    FileOpen(FileOpenTrigger),
}

impl fmt::Display for Trigger {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Trigger::ProcessStart(t) => write!(f, "ProcessStart(pid={}, comm={})", t.pid, t.comm),
            Trigger::ProcessEnd(t) => write!(f, "ProcessEnd(pid={})", t.pid),
            Trigger::OutOfMemory(t) => write!(f, "OOM(pid={}, comm={})", t.pid, t.comm),
            Trigger::FileOpen(t) => write!(f, "FileOpen(pid={}, file={})", t.pid, t.filename),
        }
    }
}

/// Exit code along with a short reason and longer explanation.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ExitReason {
    pub code: i64,
    pub term_signal: Option<u16>,
    pub reason: String,
    pub explanation: String,
}

impl ExitReason {
    pub fn success() -> Self {
        Self::from(EXIT_CODE_SUCCESS)
    }

    pub fn out_of_memory_killed() -> Self {
        Self::from(EXIT_CODE_OUT_OF_MEMORY_KILLED)
    }
}

impl fmt::Display for ExitReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.reason)
    }
}

impl From<i64> for ExitReason {
    fn from(value: i64) -> Self {
        let status = value as u16;
        let code = ((status >> 8) & 0xff) as i64;
        let signaled = (status & 0x7f) != 0;
        let term_signal = if signaled { Some(status & 0x7f) } else { None };
        Self {
            code,
            term_signal,
            reason: exit_code_reason(code),
            explanation: exit_code_explanation(code),
        }
    }
}

pub const EXIT_CODE_SUCCESS: i64 = 0;
pub const EXIT_CODE_COMMAND_NOT_INVOKED: i64 = 126;
pub const EXIT_CODE_COMMAND_NOT_FOUND: i64 = 127;
pub const EXIT_CODE_CTRL_C_KILLED: i64 = 130;
pub const EXIT_CODE_OUT_OF_MEMORY_KILLED: i64 = 137;
pub const EXIT_CODE_SIGNAL_TERMINATED: i64 = 143;

pub fn exit_code_reason(code: i64) -> String {
    match code {
        EXIT_CODE_SUCCESS => "Success".to_string(),
        EXIT_CODE_COMMAND_NOT_INVOKED => "Command Not Invoked".to_string(),
        EXIT_CODE_COMMAND_NOT_FOUND => "Command Not Found".to_string(),
        EXIT_CODE_CTRL_C_KILLED => "Terminated by Ctrl-C".to_string(),
        EXIT_CODE_OUT_OF_MEMORY_KILLED => "OOM Killed".to_string(),
        EXIT_CODE_SIGNAL_TERMINATED => "SIGTERM".to_string(),
        code if (128..=255).contains(&code) => format!("Signal {}", code),
        code if (0..=127).contains(&code) => format!("Exit code {}", code),
        code => format!("Unknown Code {}", code),
    }
}

pub fn exit_code_explanation(code: i64) -> String {
    match code {
        EXIT_CODE_SUCCESS => "Exited successfully.".to_string(),
        EXIT_CODE_COMMAND_NOT_INVOKED => {
            "Command Not Invoked: The command could not be invoked.".to_string()
        }
        EXIT_CODE_COMMAND_NOT_FOUND => {
            "Command Not Found: The command was not found.".to_string()
        }
        EXIT_CODE_CTRL_C_KILLED => "Terminated by Ctrl-C.".to_string(),
        EXIT_CODE_OUT_OF_MEMORY_KILLED => {
            "SIGKILL: The container was forcefully terminated; typically this is due to exceeding memory limits.".to_string()
        }
        EXIT_CODE_SIGNAL_TERMINATED => "SIGTERM: Graceful termination requested.".to_string(),
        code if (128..=255).contains(&code)=> format!("Terminated by signal {}.", code),
        code if (0..=127).contains(&code) => format!("Exited with code {} indicating an error in the invoked process.", code),
        code => format!("Exited with unknown code {}.", code),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration_ns(0), "N/A");
        assert_eq!(format_duration_ns(500), "500ns");
        assert_eq!(format_duration_ns(1500), "1.50µs");
        assert_eq!(format_duration_ns(1_500_000), "1.50ms");
        assert_eq!(format_duration_ns(1_500_000_000), "1.500s");
    }
}
