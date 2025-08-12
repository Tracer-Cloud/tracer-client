use std::env;
use std::process::{Command, Stdio};

#[test]
fn test_spawn() {
    // get the path of the main executable
    let exe_path = env!("CARGO_BIN_EXE_tracer-common");

    // execute the parent process - this will fork a child process, get its output, then
    // echo the same output, which we can check here
    let output = Command::new(exe_path)
        .stdout(Stdio::piped())
        .output()
        .unwrap()
        .stdout;

    assert_eq!(output, b"hello world\n");
}
