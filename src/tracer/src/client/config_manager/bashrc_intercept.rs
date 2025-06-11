use anyhow::Result;
use std::{
    fs::OpenOptions,
    io::{BufRead, BufReader, Write},
    path::PathBuf,
};

use crate::common::target_process::target_matching::TargetMatch;
use crate::common::target_process::Target;

const INTERCEPTOR_BASHRC_PATH: &str = ".config/tracer/.bashrc";
const INTERCEPTOR_SOURCE_COMMAND: &str = "source ~/.config/tracer/.bashrc";
pub const INTERCEPTOR_STDOUT_FILE: &str = "/tmp/tracerd-stdout";
pub const INTERCEPTOR_STDERR_FILE: &str = "/tmp/tracerd-stderr";
const INTERCEPTOR_STDOUT_COMMAND: &str = "exec &> >(tee >(awk 'system(\"[ ! -f /tmp/tracerd.pid ]\") == 1' >> \"/tmp/tracerd-stdout\"))\n";
const INTERCEPTOR_STDERR_COMMAND: &str = "exec 2> >(tee >(awk 'system(\"[ ! -f /tmp/tracerd.pid ]\") == 1' >> \"/tmp/tracerd-stderr\"))\n";

pub fn get_command_interceptor(
    current_tracer_exe_path: PathBuf,
    command_name: &str,
    display_name: &str,
) -> String {
    format!(
        "alias {}=\"{} & {} log-short-lived-process \\\"{}\\\"; wait\"\n",
        command_name,
        command_name,
        current_tracer_exe_path.as_os_str().to_str().unwrap(),
        display_name,
    )
}

pub fn rewrite_interceptor_bashrc_file(
    current_tracer_exe_path: PathBuf,
    targets: Vec<&Target>,
) -> Result<()> {
    let path = homedir::get_my_home()?.unwrap();

    let mut bashrc_file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(path.join(INTERCEPTOR_BASHRC_PATH))?;

    for command in targets.into_iter().map(|target| {
        let name = target.get_display_name_object();
        let command_to_alias = match &target.match_type {
            TargetMatch::ShortLivedProcessExecutable(alias) => alias.clone(),
            _ => "unknown_command".to_string(),
        };
        format!(
            "{}\n",
            get_command_interceptor(
                current_tracer_exe_path.clone(),
                &command_to_alias,
                &name.get_display_name(&command_to_alias, &[])
            )
        )
    }) {
        bashrc_file.write_all(command.as_bytes()).unwrap();
    }

    bashrc_file
        .write_all(INTERCEPTOR_STDOUT_COMMAND.as_bytes())
        .unwrap();

    bashrc_file
        .write_all(INTERCEPTOR_STDERR_COMMAND.as_bytes())
        .unwrap();

    Ok(())
}

pub fn modify_bashrc_file(bashrc_file_path: &str) -> Result<()> {
    let path = homedir::get_my_home()?.unwrap();

    let mut bashrc_file = OpenOptions::new()
        .read(true)
        .append(true)
        .open(path.join(bashrc_file_path))?;

    let reader = BufReader::new(&bashrc_file);
    for line in reader.lines() {
        let line = line.unwrap();
        if line.contains(INTERCEPTOR_SOURCE_COMMAND) {
            return Ok(());
        }
    }

    bashrc_file
        .write_all(INTERCEPTOR_SOURCE_COMMAND.as_bytes())
        .unwrap();

    Ok(())
}
