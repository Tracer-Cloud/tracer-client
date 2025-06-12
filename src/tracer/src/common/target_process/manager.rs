use crate::common::target_process::{Target, TargetMatchable};
use tracer_ebpf::{EbpfEvent, SchedSchedProcessExecPayload};

#[derive(Clone, Debug)]
pub struct TargetManager {
    pub targets: Vec<Target>,
    pub blacklist: Vec<Target>,
}

impl TargetManager {
    pub fn new(targets: Vec<Target>, blacklist: Vec<Target>) -> Self {
        Self { targets, blacklist }
    }

    /// Returns the matching target if it's not blacklisted
    pub fn get_target_match(
        &self,
        event: &EbpfEvent<SchedSchedProcessExecPayload>,
    ) -> Option<&Target> {
        // Skip blacklisted processes
        if self.blacklist.iter().any(|b| b.matches_process(event)) {
            tracing::error!(
                "blocking process: {} | pid: {} | argv: {:?}",
                event.header.comm,
                event.header.pid,
                event.payload.argv
            );
            return None;
        }

        // Return first matching target
        self.targets.iter().find(|t| t.matches_process(event))
    }
}

//TODO add tests related to targets
#[cfg(test)]
mod tests {
    use crate::common::target_process::manager::TargetManager;
    use crate::common::target_process::target_matching::{CommandContainsStruct, TargetMatch};
    use crate::common::target_process::Target;
    use tracer_ebpf::{EbpfEvent, EventHeader, EventType, SchedSchedProcessExecPayload};

    fn dummy_process_exec_event(
        name: &str,
        cmd: &str,
        _path: &str,
    ) -> EbpfEvent<SchedSchedProcessExecPayload> {
        let argv: Vec<String> = cmd.split_whitespace().map(String::from).collect();

        EbpfEvent::<SchedSchedProcessExecPayload> {
            header: EventHeader {
                event_id: 1,
                event_type: EventType::SchedSchedProcessExec,
                timestamp_ns: 0,
                pid: 1,
                ppid: 0,
                upid: 1,
                uppid: 0,
                comm: name.to_string(),
            },
            payload: SchedSchedProcessExecPayload { argv },
        }
    }

    #[test]
    fn test_blacklist_excludes_match() {
        let blacklist = vec![Target::new(TargetMatch::CommandContains(
            CommandContainsStruct {
                process_name: None,
                command_content: "spack".to_string(),
            },
        ))];
        let targets = vec![Target::new(TargetMatch::ProcessName("fastqc".to_string()))];

        let mgr = TargetManager::new(targets, blacklist);
        let proc =
            dummy_process_exec_event("fastqc", "spack activate && fastqc", "/usr/bin/fastqc");

        assert!(mgr.get_target_match(&proc).is_none());
    }

    #[test]
    fn test_target_match_without_blacklist() {
        let mgr = TargetManager::new(
            vec![Target::new(TargetMatch::ProcessName("fastqc".to_string()))],
            vec![],
        );
        let process = dummy_process_exec_event("fastqc", "fastqc file.fq", "/usr/bin/fastqc");
        assert!(mgr.get_target_match(&process).is_some());
    }
}
