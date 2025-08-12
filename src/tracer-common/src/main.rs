use std::path::PathBuf;
use std::{env, fs};
use tempfile::TempDir;
use tracer_common::secure::spawn::*;

fn main() {
    let (exe_path, inode) = resolve_exe_path();
    
    let args = env::args().collect::<Vec<_>>();

    if args.len() > 1 {
        // this is the child process
        let outfile = PathBuf::from(args.get(1).unwrap());
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
        let mut child = spawn_child_default(&[&outfile.as_os_str().to_string_lossy()]).unwrap();
        let exit = child.wait().unwrap();
        if !exit.success() {
            panic!("child process failed");
        }
        if !outfile.exists() {
            panic!("child process did not create output file")
        }
        let msg = fs::read_to_string(outfile).unwrap();
        println!("{}", msg);
    }
}
