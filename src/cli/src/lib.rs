use tracer_common::types::cli::{
    interactive::InteractiveInitArgs,
    params::{FinalizedInitArgs, TracerCliInitArgs},
};

pub mod commands;
pub mod logging;
pub mod nondaemon_commands;
pub mod process_command;
pub mod utils;

/// Runs tracer init in interactive mode
pub fn init_command_interactive_mode(cli_args: TracerCliInitArgs) -> FinalizedInitArgs {
    InteractiveInitArgs::from_partial(cli_args)
        .prompt_missing()
        .into_cli_args()
}
