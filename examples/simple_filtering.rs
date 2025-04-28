use std::fs::File;
use std::io::{BufWriter, Write};
use std::thread::sleep;
use std::time::{Duration, Instant};
use sysinfo::{ProcessExt, System, SystemExt};

#[derive(Debug)]
struct SimpleTarget {
    name: String,
}

impl SimpleTarget {
    fn matches(&self, process_name: &str, command: &str) -> bool {
        let pname = process_name.to_lowercase();
        let cmd = command.to_lowercase();
        pname.contains(&self.name) || cmd.contains(&self.name)
    }
}

fn main() {
    let duration_to_run = Duration::from_secs(5 * 60); // 5 minutes
    let poll_interval = Duration::from_secs(5); // check every 5 seconds

    let start_time = Instant::now();
    let mut system = System::new_all();

    // File to write captured data
    let file = File::create("process_capture.txt").expect("Failed to create file");
    let mut writer = BufWriter::new(file);

    // Write headers
    writeln!(
        writer,
        "{:<8} {:<20} {}",
        "PID", "Process Name", "Command Line"
    )
    .unwrap();

    let targets = vec![
        SimpleTarget {
            name: "star".to_string(),
        },
        SimpleTarget {
            name: "fastqc".to_string(),
        },
        SimpleTarget {
            name: "salmon".to_string(),
        },
        SimpleTarget {
            name: "bedtools".to_string(),
        },
        SimpleTarget {
            name: "nextflow".to_string(),
        },
    ];

    while start_time.elapsed() < duration_to_run {
        system.refresh_processes();

        for (pid, process) in system.processes() {
            let process_name = process.name();
            let command = process.cmd().join(" ");

            let is_target = targets
                .iter()
                .any(|target| target.matches(process_name, &command));
            let is_thread_exception = process_name.to_lowercase().contains("thread-");

            if is_target || is_thread_exception {
                writeln!(writer, "{:<8} {:<20} {}", pid, process_name, command).unwrap();
            }
        }

        writer.flush().unwrap(); // Flush after every round
        sleep(poll_interval);
    }

    println!("Process capture completed. Saved to 'process_capture.txt'");
}
