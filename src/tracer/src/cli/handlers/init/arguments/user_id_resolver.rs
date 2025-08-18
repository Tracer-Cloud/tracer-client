use crate::utils::user_id_resolution::extract_user_id;

use super::super::user_prompts::{print_help, UserPrompts};
use super::arguments::PromptMode;

/// Resolves user ID from various sources using functional programming approach
/// Uses extract_user_id with shell config file reading and comprehensive Sentry instrumentation
pub fn resolve_user_id(current_user_id: Option<String>, prompt_mode: &PromptMode) -> String {
    // First try extracting user_id which handles all fallback strategies
    match extract_user_id(current_user_id) {
        Ok(user_id) => {
            // If we have a user_id and prompts are required, confirm with user
            match prompt_mode {
                PromptMode::Required => {
                    UserPrompts::prompt_for_user_id(Some(&user_id))
                }
                _ => user_id
            }
        }
        Err(_) => {
            // If user_id extraction fails, fall back to prompting if allowed
            match prompt_mode {
                PromptMode::Minimal | PromptMode::Required => {
                    UserPrompts::prompt_for_user_id(None)
                }
                PromptMode::None => {
                    print_help().expect("Failed to get user ID from any source")
                }
            }
        }
    }
}



#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_resolve_user_id_with_existing_id() {
        // Test that provided user_id is used when available
        let result = resolve_user_id(Some("test_user".to_string()), &PromptMode::None);
        assert_eq!(result, "test_user");
    }

    #[test]
    fn test_resolve_user_id_with_env_var() {
        // Test that environment variable is used as fallback
        env::set_var("TRACER_USER_ID", "env_test_user");
        let result = resolve_user_id(None, &PromptMode::None);
        assert_eq!(result, "env_test_user");
        env::remove_var("TRACER_USER_ID");
    }
}
