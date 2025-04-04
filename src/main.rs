use tracer::cli::process_cli;

#[tokio::main]
pub async fn main() {
    if let Err(err) = process_cli().await {
        eprintln!("Error processing Cli: {err}");
    }
}
