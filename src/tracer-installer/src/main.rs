use checks::CheckManager;

mod checks;

#[tokio::main]
async fn main() {
    println!("Welcome to tracer rust installer");
    let requirements = CheckManager::new();
    requirements.run_all().await;
}
