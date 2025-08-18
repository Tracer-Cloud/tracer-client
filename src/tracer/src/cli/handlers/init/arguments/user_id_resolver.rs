use crate::utils::env;
use crate::warning_message;
use colored::Colorize;

use super::super::user_prompts::{print_help, UserPrompts};
use super::arguments::{PromptMode, USERNAME_ENV_VAR};

/// Resolves user ID from various sources using functional programming approach
pub fn resolve_user_id(current_user_id: Option<String>, prompt_mode: &PromptMode) -> String {
    let username = env::get_env_var(USERNAME_ENV_VAR);

    resolve_user_id_with_sources(current_user_id, prompt_mode, username)
        .or_else(print_help)
        .expect("Failed to get user ID from environment variable, command line, or prompt")
}

/// Pure function that resolves user ID based on inputs
fn resolve_user_id_with_sources(
    current_user_id: Option<String>,
    prompt_mode: &PromptMode,
    username: Option<String>,
) -> Option<String> {
    match (current_user_id, prompt_mode) {
        (Some(user_id), PromptMode::Required) => {
            // Only prompt for confirmation in Required mode
            Some(UserPrompts::prompt_for_user_id(Some(&user_id)))
        }
        (Some(user_id), _) => Some(user_id),
        (None, PromptMode::Minimal | PromptMode::Required) => {
            Some(UserPrompts::prompt_for_user_id(username.as_deref()))
        }
        (None, PromptMode::None) => handle_no_user_id_fallback(username),
    }
}

/// Handles fallback when no user ID is provided and prompts are disabled
fn handle_no_user_id_fallback(username: Option<String>) -> Option<String> {
    // TODO: remove this once we can source the user ID from the credentials file
    if let Some(ref username_val) = username {
        warning_message!(
            "Failed to get user ID from environment variable, command line, or prompt. \
            defaulting to the system username '{}', which may not be your Tracer user ID! \
            Please set the TRACER_USER_ID environment variable or specify the --user-id \
            option.",
            username_val
        );
    }
    username
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_user_id_with_existing_id_non_required() {
        let result =
            resolve_user_id_with_sources(Some("test_user".to_string()), &PromptMode::None, None);
        assert_eq!(result, Some("test_user".to_string()));
    }

    #[test]
    fn test_resolve_user_id_fallback_to_username() {
        let result =
            resolve_user_id_with_sources(None, &PromptMode::None, Some("system_user".to_string()));
        assert_eq!(result, Some("system_user".to_string()));
    }

    #[test]
    fn test_resolve_user_id_no_fallback() {
        let result = resolve_user_id_with_sources(None, &PromptMode::None, None);
        assert_eq!(result, None);
    }
}
