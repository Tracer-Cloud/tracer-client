use anyhow::Context;
use tracer_cli::process_command::process_cli;

pub fn main() -> anyhow::Result<()> {
    process_cli().context("Can't process CLI command")?;
    Ok(())
}
