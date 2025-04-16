#[derive(Debug)]
pub enum Trigger {
    Start {
        pid: u32,
        file_name: String,
        argv: Vec<String>,
    }
}
