#[derive(Debug)]
pub enum Trigger {
    Start {
        pid: u32,
        comm: String,
        file_name: String,
        argv: Vec<String>,
    },
}
