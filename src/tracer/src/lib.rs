use crate::process_identification::types::cli::{
    interactive::InteractiveInitArgs,
    params::{FinalizedInitArgs, TracerCliInitArgs},
};

pub mod client;
pub mod cloud_providers;
pub mod commands;
pub mod config;
pub mod constants;
pub mod daemon;
pub mod extracts;
pub mod logging;
pub mod nondaemon_commands;
pub mod process_command;
pub mod process_identification;
pub mod utils;
/// Runs tracer init in interactive mode
pub fn init_command_interactive_mode(cli_args: TracerCliInitArgs) -> FinalizedInitArgs {
    InteractiveInitArgs::from_partial(cli_args)
        .prompt_missing()
        .into_cli_args()
}
