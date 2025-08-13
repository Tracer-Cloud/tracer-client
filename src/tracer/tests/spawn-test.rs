#[cfg(feature = "test-bins")]
#[test]
fn test_spawn() {
    use std::env;
    use std::path::PathBuf;
    use std::process::{Command, Stdio};
    use tracer::utils::spawn::get_inode;

    // get the path of the main executable
    let exe_path = env!("CARGO_BIN_EXE_tracer-test-spawn");

    // execute the parent process - this will fork a child process, get its output, then
    // echo the same output, which we can check here
    // the output
    let output = Command::new(exe_path)
        .stdout(Stdio::piped())
        .output()
        .unwrap()
        .stdout;

    let output = String::from_utf8(output).unwrap();
    let output = output.lines().last().unwrap();
    let parts = output.split("|").collect::<Vec<_>>();

    if parts.len() != 3 {
        panic!("unexpected output: {}", output);
    }
    assert_eq!(parts[0], "child");
    assert_eq!(parts[1], exe_path);

    let expected_inode = parts[2].trim().parse::<u64>().ok();
    let actual_inode = get_inode(&PathBuf::from(exe_path));

    match (expected_inode, actual_inode) {
        (Some(expected), Some(actual)) => assert_eq!(expected, actual),
        (None, None) => (),
        _ => panic!(
            "expected and actual inode mismatch: {:?} != {:?}",
            expected_inode, actual_inode
        ),
    }
}

#[cfg(not(feature = "test-bins"))]
#[test]
fn test_spawn() {
    println!("test not run - requires test-bins feature");
}
