#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct ProcessTrigger {
    pub pid: usize,
    pub ppid: usize,
    pub comm: String,
    pub file_name: String,
    pub argv: Vec<String>,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct FinishTrigger {
    pub pid: usize,
}

#[derive(Debug, Clone)]
pub enum Trigger {
    Start(ProcessTrigger),
    Finish(FinishTrigger),
}
