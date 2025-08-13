use clap::Parser;
use std::path::PathBuf;
use std::time::Duration;
use std::{fs, thread};
use tempfile::TempDir;
use tracer_common::secure::spawn::*;

#[derive(Parser, Clone)]
#[clap(name = "tracer-common")]
pub struct Cli {
    #[clap(long)]
    pub outfile: Option<PathBuf>,
}

fn main() {
    let (exe_path, inode) = resolve_exe_path();

    let cli = Cli::parse();

    if let Some(outfile) = &cli.outfile {
        // this is the child process
        // write out the exe path and inode
        std::fs::write(
            outfile,
            format!(
                "child|{}|{}",
                exe_path.display(),
                inode.map(|i| format!("{}", i)).unwrap_or("".to_string())
            ),
        )
        .unwrap();
    } else {
        // this is the parent process
        let workdir = TempDir::new().unwrap();
        let outfile = workdir.path().join("test.txt");
        let _child_pid =
            spawn_child(&["--outfile", &outfile.as_os_str().to_string_lossy()]).unwrap() as usize;

        // wait for the child to finish
        thread::sleep(Duration::from_secs(3));

        if !outfile.exists() {
            panic!("child process did not create output file")
        }

        let msg = fs::read_to_string(outfile).unwrap();
        println!("{}", msg);
    }
}
