use checks::CheckManager;
use installer::run_installer;

mod checks;
mod installer;
mod types;

#[tokio::main]
async fn main() {
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");

    println!("Welcome to tracer rust installer");
    let requirements = CheckManager::new();
    requirements.run_all().await;

    run_installer().await;
}
