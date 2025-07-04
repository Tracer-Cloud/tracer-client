use crate::process_identification::types::cli::{
    interactive::InteractiveInitArgs,
    params::{FinalizedInitArgs, TracerCliInitArgs},
};

pub mod cli;
pub mod client;
pub mod cloud_providers;
pub mod config;
pub mod constants;
pub mod daemon;
pub mod extracts;
pub mod process_identification;
pub mod utils;

/// Runs tracer init in interactive mode
pub fn init_command_interactive_mode(cli_args: TracerCliInitArgs) -> FinalizedInitArgs {
    InteractiveInitArgs::from_partial(cli_args)
        .prompt_missing()
        .into_cli_args()
}
