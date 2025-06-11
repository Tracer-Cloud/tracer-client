use tracer_ebpf::ebpf_trigger::ProcessStartTrigger;

use super::{Target, TargetMatchable};

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
    pub fn get_target_match(&self, process: &ProcessStartTrigger) -> Option<&Target> {
        // Skip blacklisted processes
        if self.blacklist.iter().any(|b| b.matches_process(process)) {
            tracing::error!(
                "blocking process: {} | path: {} | argv: {:?}",
                process.comm,
                process.file_name,
                process.argv
            );
            return None;
        }

        // Return first matching target
        self.targets.iter().find(|t| t.matches_process(process))
    }
}

//TODO add tests related to targets
#[cfg(test)]
mod tests {
    use crate::common::target_process::manager::TargetManager;
    use crate::common::target_process::target_matching::{CommandContainsStruct, TargetMatch};
    use crate::common::target_process::Target;
    use tracer_ebpf::ebpf_trigger::ProcessStartTrigger;

    fn dummy_process(name: &str, cmd: &str, path: &str) -> ProcessStartTrigger {
        ProcessStartTrigger {
            pid: 1,
            ppid: 0,
            comm: name.to_string(),
            argv: cmd.split_whitespace().map(String::from).collect(),
            file_name: path.to_string(),
            started_at: chrono::Utc::now(),
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
        let proc = dummy_process("fastqc", "spack activate && fastqc", "/usr/bin/fastqc");

        assert!(mgr.get_target_match(&proc).is_none());
    }

    #[test]
    fn test_target_match_without_blacklist() {
        let mgr = TargetManager::new(
            vec![Target::new(TargetMatch::ProcessName("fastqc".to_string()))],
            vec![],
        );
        let process = dummy_process("fastqc", "fastqc file.fq", "/usr/bin/fastqc");
        assert!(mgr.get_target_match(&process).is_some());
    }
}
