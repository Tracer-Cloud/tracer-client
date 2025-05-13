use bollard::container::{InspectContainerOptions, ListContainersOptions};
use bollard::container::{LogOutput, LogsOptions};
use bollard::Docker;
use futures_util::stream::StreamExt;
use sqlx::PgPool;
use std::io::Write;
use std::process::Command;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command as TokioCommand;
use tokio::time::{sleep, Duration};

pub async fn monitor_container(docker: &Docker, container_prefix: &str) {
    let options = ListContainersOptions::<String> {
        all: true,
        ..Default::default()
    };

    let containers: Vec<String> = docker
        .list_containers(Some(options))
        .await
        .expect("Failed to get containers")
        .iter()
        .filter_map(|ex| {
            ex.names.as_ref().and_then(|names| {
                names
                    .iter()
                    .find(|name| name.contains(container_prefix))
                    .map(|name| name.trim_start_matches('/').to_string())
            })
        })
        .collect();

    if containers.is_empty() {
        println!(
            "No running containers found with prefix: {}",
            container_prefix
        );
        return;
    }

    loop {
        let mut all_stopped = true;

        for container_name in &containers {
            if let Ok(container_info) = docker
                .inspect_container(container_name, Some(InspectContainerOptions::default()))
                .await
            {
                if let Some(state) = container_info.state {
                    if state.running.unwrap_or(false) {
                        all_stopped = false;
                    }
                }
            }
        }

        if all_stopped {
            break; // All containers have stopped, exit loop
        }

        sleep(Duration::from_secs(2)).await;
    }

    println!("All monitored containers have finished executing.");
}

pub async fn start_docker_compose(profile: &str) {
    let output = Command::new("docker")
        .arg("compose")
        .arg("--profile")
        .arg(profile)
        .arg("up")
        .arg("-d") // Detached mode
        .output()
        .expect("Failed to start Docker Compose");

    if !output.status.success() {
        eprintln!(
            "Failed to start Docker Compose: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

pub async fn end_docker_compose(profile: &str) {
    let output = Command::new("docker")
        .arg("compose")
        .arg("--profile")
        .arg(profile)
        .arg("down")
        .output()
        .expect("Failed to start Docker Compose");

    if !output.status.success() {
        eprintln!(
            "Failed to end Docker Compose: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

async fn wait_for_db_ready(db_url: &str) -> PgPool {
    let mut attempts = 10; // Max retries
    while attempts > 0 {
        match PgPool::connect(db_url).await {
            Ok(pool) => {
                println!("Database is ready!");
                return pool;
            }
            Err(e) => {
                println!("⏳ Waiting for DB to be ready... ({})", e);
                sleep(Duration::from_secs(2)).await;
                attempts -= 1;
            }
        }
    }
    panic!("failed to start!");
}

pub async fn setup_db(db_url: &str) -> PgPool {
    println!("Running migrations...");
    // Run migrations
    let pool = wait_for_db_ready(db_url).await;

    sqlx::query("DROP TABLE IF EXISTS batch_jobs_logs")
        .execute(&pool)
        .await
        .expect("Failed to drop batch_jobs_logs table");

    // Delete the migration table if it exists
    sqlx::query("DROP TABLE IF EXISTS _sqlx_migrations")
        .execute(&pool)
        .await
        .expect("Failed to drop migration table");

    sqlx::migrate!("../migrations")
        .run(&pool)
        .await
        .expect("Failed to run migration");

    sleep(Duration::from_millis(100)).await;

    pool
}

pub async fn print_all_container_logs(docker: &Docker) {
    let containers = docker
        .list_containers(Some(ListContainersOptions::<String> {
            all: true,
            ..Default::default()
        }))
        .await
        .expect("Failed to list containers");

    for container in containers {
        if let Some(names) = container.names {
            let name = names
                .first()
                .map(|n| n.trim_start_matches('/'))
                .unwrap_or("unknown");

            let options = Some(LogsOptions::<String> {
                stdout: true,
                stderr: true,
                follow: false,
                tail: "all".into(),
                ..Default::default()
            });

            println!("\n📦 Logs for container: `{}`", name);

            let mut logs = docker.logs(name, options);

            while let Some(log_result) = logs.next().await {
                match log_result {
                    Ok(LogOutput::StdOut { message }) | Ok(LogOutput::StdErr { message }) => {
                        print!("{}", String::from_utf8_lossy(&message));
                        std::io::stdout().flush().unwrap();
                    }
                    Ok(_) => {}
                    Err(err) => {
                        eprintln!("❌ Error reading logs for `{}`: {}", name, err);
                        break;
                    }
                }
            }
        }
    }
}

pub async fn dump_container_file_for_all_matching(
    docker: &Docker,
    container_prefix: &str,
    file_path: &str,
) {
    let containers = docker
        .list_containers(Some(ListContainersOptions::<String> {
            all: true,
            ..Default::default()
        }))
        .await
        .expect("Failed to list containers");

    let mut tasks = vec![];

    for container in containers {
        let container_name = container
            .names
            .as_ref()
            .and_then(|names| names.first())
            .map(|name| name.trim_start_matches('/').to_string())
            .unwrap_or_else(|| "unknown".to_string());

        let image_name = container.image.unwrap_or_default();

        if container_name.contains(container_prefix) || image_name.contains(container_prefix) {
            let name = container_name.clone();
            let path = file_path.to_string();
            let docker = docker.clone();

            tasks.push(tokio::spawn(async move {
                loop {
                    let is_running = match docker
                        .inspect_container(&name, None)
                        .await
                        .ok()
                        .and_then(|info| info.state)
                    {
                        Some(state) => state.running.unwrap_or(false),
                        None => false,
                    };

                    if !is_running {
                        break;
                    }

                    let mut cmd = TokioCommand::new("docker")
                        .arg("exec")
                        .arg(&name)
                        .arg("cat")
                        .arg(&path)
                        .stdout(std::process::Stdio::piped())
                        .spawn()
                        .expect("Failed to exec docker");

                    if let Some(stdout) = cmd.stdout.take() {
                        let mut reader = BufReader::new(stdout).lines();

                        while let Ok(Some(line)) = reader.next_line().await {
                            println!("📄 [{}:{}] {}", name, path, line);
                        }
                    }

                    sleep(Duration::from_secs(2)).await;
                }

                println!("✅ Done streaming `{}` for container `{}`", path, name);
            }));
        }
    }

    // Wait for all tasks to complete
    for task in tasks {
        let _ = task.await;
    }
}
