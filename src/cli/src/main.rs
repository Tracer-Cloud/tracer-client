use tracer_cli::process_command::process_cli;

pub fn main() {
    if let Err(err) = process_cli() {
        eprintln!("Error processing Cli: {err}");
    }
}
