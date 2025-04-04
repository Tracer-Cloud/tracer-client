use tracer::cli::process_cli;

pub fn main() {
    if let Err(err) = process_cli() {
        eprintln!("Error processing Cli: {err}");
    }
}
