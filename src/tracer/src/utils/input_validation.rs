use clap::builder::TypedValueParser;
use clap::error::{ContextKind, ContextValue, Error, ErrorKind};
use dialoguer::theme::ColorfulTheme;
use dialoguer::Input;
use std::collections::HashSet;
use std::sync::LazyLock;

#[derive(Clone)]
pub struct StringValueParser;

impl TypedValueParser for StringValueParser {
    type Value = String;

    fn parse_ref(
        &self,
        cmd: &clap::Command,
        arg: Option<&clap::Arg>,
        value: &std::ffi::OsStr,
    ) -> Result<Self::Value, clap::Error> {
        let field = arg.map(|arg| arg.to_string()).unwrap_or("unknown".into());
        let str_value = match value.to_str() {
            Some(value) => value,
            None => {
                let mut err = Error::new(ErrorKind::InvalidUtf8).with_cmd(cmd);
                err.insert(ContextKind::InvalidArg, ContextValue::String(field));
                return Err(err);
            }
        };
        match validate_input_string(&str_value, &field) {
            Ok(_) => Ok(str_value.to_string()),
            Err(e) => Err(Error::raw(ErrorKind::ValueValidation, e)),
        }
    }
}

const INVALID_CHAR_ARRAY: [char; 91] = [
    // Control characters
    '\0', '\x01', '\x02', '\x03', '\x04', '\x05', '\x06', '\x07', // 0x00-0x07
    '\x08', '\x09', '\x0A', '\x0B', '\x0C', '\x0D', '\x0E', '\x0F', // 0x08-0x0F
    '\x10', '\x11', '\x12', '\x13', '\x14', '\x15', '\x16', '\x17', // 0x10-0x17
    '\x18', '\x19', '\x1A', '\x1B', '\x1C', '\x1D', '\x1E', '\x1F', // 0x18-0x1F
    '\x7F', // DEL character
    // Path separators
    '\\', '/', // SQL injection characters
    '\'', '"', ';', '`', // Shell injection characters
    '&', '|', '$', '(', ')', '{', '}', '[', ']', '*', '?', '~', '!', '@', '#', '%', '^', '+',
    '=', // Unicode control characters
    '\u{0000}', '\u{0001}', '\u{0002}', '\u{0003}', '\u{0004}', '\u{0005}', '\u{0006}', '\u{0007}',
    '\u{0008}', '\u{0009}', '\u{000A}', '\u{000B}', '\u{000C}', '\u{000D}', '\u{000E}', '\u{000F}',
    '\u{0010}', '\u{0011}', '\u{0012}', '\u{0013}', '\u{0014}', '\u{0015}', '\u{0016}', '\u{0017}',
    '\u{0018}', '\u{0019}', '\u{001A}', '\u{001B}', '\u{001C}', '\u{001D}', '\u{001E}', '\u{001F}',
    '\u{007F}', // DEL
];

static INVALID_CHARS: LazyLock<HashSet<char>> = LazyLock::new(
    || // Control characters (0x00-0x1F, 0x7F) and other problematic characters
    HashSet::from(INVALID_CHAR_ARRAY),
);

const INVALID_PATH_PATTERNS: [&str; 35] = [
    "../",
    "..\\",
    "./",
    ".\\",
    "/etc/",
    "\\windows\\",
    "/proc/",
    "/sys/",
    "/dev/",
    "~/.",
    "~/",
    "C:\\",
    "D:\\",
    "E:\\",
    "F:\\",
    "G:\\",
    "H:\\",
    "I:\\",
    "J:\\",
    "K:\\",
    "L:\\",
    "M:\\",
    "N:\\",
    "O:\\",
    "P:\\",
    "Q:\\",
    "R:\\",
    "S:\\",
    "T:\\",
    "U:\\",
    "V:\\",
    "W:\\",
    "X:\\",
    "Y:\\",
    "Z:\\",
];

/// Validates that a string doesn't contain any problematic characters for database safety and security
///
/// This function checks for:
/// - Control characters (0x00-0x1F, 0x7F)
/// - Path separators (\, /)
/// - SQL injection characters (', ", ;, `)
/// - Shell injection characters (&, |, $, (, ), {, }, [, ], *, ?, ~, !, @, #, %, ^, +, =)
/// - Empty or whitespace-only strings
/// - Strings that are too long (max 255 characters)
pub fn validate_input_string(input: &str, field: &str) -> Result<(), String> {
    // Check for empty or whitespace-only strings
    if input.trim().is_empty() {
        return Err(format!(
            "Value for option '{}' cannot be empty or contain only whitespace.",
            field
        ));
    }

    // Check for maximum length
    if input.len() > 255 {
        return Err(format!(
            "Value '{}' for option '{}; is too long (maximum 255 characters allowed).",
            input, field
        ));
    }

    for (i, ch) in input.char_indices() {
        if INVALID_CHARS.contains(&ch) {
            return Err(format!(
                "Value '{}' for option '{}' contains invalid character '{}' at position {}.
                Control characters, escape characters, and path separators are not allowed.",
                input,
                field,
                ch.escape_default().collect::<String>(),
                i + 1,
            ));
        }
    }

    // Check for common file path patterns that could be dangerous
    for pattern in INVALID_PATH_PATTERNS {
        if input.to_lowercase().contains(pattern) {
            return Err(format!(
                "Invalid path pattern '{}' found in {}. Path traversal patterns are not allowed.",
                pattern, field
            ));
        }
    }

    Ok(())
}

/// Validates and returns the input string, or prompts for a new one if invalid
pub fn get_validated_input(
    theme: &ColorfulTheme,
    prompt: &str,
    default: Option<&str>,
    field_name: &str,
) -> String {
    loop {
        let input = if let Some(default_val) = default {
            Input::with_theme(theme)
                .with_prompt(prompt)
                .default(default_val.to_string())
                .interact_text()
                .inspect_err(|e| panic!("Error while prompting for {}: {e}", field_name))
                .unwrap()
        } else {
            Input::with_theme(theme)
                .with_prompt(prompt)
                .interact_text()
                .inspect_err(|e| panic!("Error while prompting for {}: {e}", field_name))
                .unwrap()
        };

        match validate_input_string(&input, field_name) {
            Ok(_) => return input,
            Err(error_msg) => {
                eprintln!("❌ {}", error_msg);
                eprintln!("Please enter a valid value without control characters, escape characters, path separators.");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_input_string_valid_inputs() {
        // Test valid inputs
        assert!(validate_input_string("valid_pipeline_name", "test").is_ok());
        assert!(validate_input_string("RNA-seq_analysis_v1", "test").is_ok());
        assert!(validate_input_string("scRNA-seq_2024", "test").is_ok());
        assert!(validate_input_string("my-pipeline-123", "test").is_ok());
        assert!(validate_input_string("pipeline_with_underscores", "test").is_ok());
        assert!(validate_input_string("normal text with spaces", "test").is_ok());
        assert!(validate_input_string("text-with-dashes", "test").is_ok());
        assert!(validate_input_string("text_with_underscores", "test").is_ok());
        assert!(validate_input_string("text123with456numbers", "test").is_ok());
    }

    #[test]
    fn test_validate_input_string_invalid_chars() {
        // Test invalid characters
        assert!(validate_input_string("pipeline\\name", "test").is_err());
        assert!(validate_input_string("pipeline/name", "test").is_err());
        assert!(validate_input_string("pipeline'name", "test").is_err());
        assert!(validate_input_string("pipeline\"name", "test").is_err());
        assert!(validate_input_string("pipeline;name", "test").is_err());
        assert!(validate_input_string("pipeline`name", "test").is_err());
        assert!(validate_input_string("pipeline\nname", "test").is_err());
        assert!(validate_input_string("pipeline\rname", "test").is_err());
        assert!(validate_input_string("pipeline\tname", "test").is_err());
        assert!(validate_input_string("pipeline&name", "test").is_err());
        assert!(validate_input_string("pipeline|name", "test").is_err());
        assert!(validate_input_string("pipeline$name", "test").is_err());
        assert!(validate_input_string("pipeline(name", "test").is_err());
        assert!(validate_input_string("pipeline)name", "test").is_err());
        assert!(validate_input_string("pipeline{name", "test").is_err());
        assert!(validate_input_string("pipeline}name", "test").is_err());
        assert!(validate_input_string("pipeline[name", "test").is_err());
        assert!(validate_input_string("pipeline]name", "test").is_err());
        assert!(validate_input_string("pipeline*name", "test").is_err());
        assert!(validate_input_string("pipeline?name", "test").is_err());
        assert!(validate_input_string("pipeline~name", "test").is_err());
        assert!(validate_input_string("pipeline!name", "test").is_err());
        assert!(validate_input_string("pipeline@name", "test").is_err());
        assert!(validate_input_string("pipeline#name", "test").is_err());
        assert!(validate_input_string("pipeline%name", "test").is_err());
        assert!(validate_input_string("pipeline^name", "test").is_err());
        assert!(validate_input_string("pipeline+name", "test").is_err());
        assert!(validate_input_string("pipeline=name", "test").is_err());
    }

    #[test]
    fn test_validate_input_string_control_chars() {
        // Test control characters
        assert!(validate_input_string("pipeline\x00name", "test").is_err());
        assert!(validate_input_string("pipeline\x01name", "test").is_err());
        assert!(validate_input_string("pipeline\x02name", "test").is_err());
        assert!(validate_input_string("pipeline\x1Fname", "test").is_err());
        assert!(validate_input_string("pipeline\x7Fname", "test").is_err());
    }

    #[test]
    fn test_validate_input_string_path_traversal() {
        // Test path traversal patterns
        assert!(validate_input_string("pipeline../name", "test").is_err());
        assert!(validate_input_string("pipeline..\\name", "test").is_err());
        assert!(validate_input_string("pipeline./name", "test").is_err());
        assert!(validate_input_string("pipeline.\\name", "test").is_err());
        assert!(validate_input_string("pipeline/etc/name", "test").is_err());
        assert!(validate_input_string("pipeline\\windows\\name", "test").is_err());
        assert!(validate_input_string("pipeline/proc/name", "test").is_err());
        assert!(validate_input_string("pipeline/sys/name", "test").is_err());
        assert!(validate_input_string("pipeline/dev/name", "test").is_err());
        assert!(validate_input_string("pipeline~/.name", "test").is_err());
        assert!(validate_input_string("pipeline~/name", "test").is_err());
        assert!(validate_input_string("pipelineC:\\name", "test").is_err());
    }

    #[test]
    fn test_validate_input_string_empty_and_whitespace() {
        // Test empty and whitespace-only strings
        assert!(validate_input_string("", "test").is_err());
        assert!(validate_input_string("   ", "test").is_err());
        assert!(validate_input_string("\t", "test").is_err());
        assert!(validate_input_string("\n", "test").is_err());
        assert!(validate_input_string("\r", "test").is_err());
        assert!(validate_input_string(" \t\n\r ", "test").is_err());
    }

    #[test]
    fn test_validate_input_string_length() {
        // Test length validation
        let long_string = "a".repeat(256);
        assert!(validate_input_string(&long_string, "test").is_err());

        let max_length_string = "a".repeat(255);
        assert!(validate_input_string(&max_length_string, "test").is_ok());
    }

    #[test]
    fn test_validate_input_string_edge_cases() {
        // Test edge cases that should be valid
        assert!(validate_input_string("a", "test").is_ok());
        assert!(validate_input_string("1", "test").is_ok());
        assert!(validate_input_string("_", "test").is_ok());
        assert!(validate_input_string("-", "test").is_ok());
        assert!(validate_input_string("normal text", "test").is_ok());
        assert!(validate_input_string("text-with-dashes-and_underscores", "test").is_ok());
        assert!(validate_input_string("text123with456numbers789", "test").is_ok());
    }

    #[test]
    fn test_validate_input_string_common_words_allowed() {
        // Test that common words that were previously blocked are now allowed
        assert!(validate_input_string("local", "test").is_ok());
        assert!(validate_input_string("nc", "test").is_ok());
        assert!(validate_input_string("or", "test").is_ok());
        assert!(validate_input_string("and", "test").is_ok());
        assert!(validate_input_string("function", "test").is_ok());
        assert!(validate_input_string("return", "test").is_ok());
        assert!(validate_input_string("exit", "test").is_ok());
        assert!(validate_input_string("break", "test").is_ok());
        assert!(validate_input_string("continue", "test").is_ok());
        assert!(validate_input_string("shift", "test").is_ok());
        assert!(validate_input_string("getopts", "test").is_ok());
        assert!(validate_input_string("first", "test").is_ok());
        assert!(validate_input_string("last", "test").is_ok());
        assert!(validate_input_string("skip", "test").is_ok());
        assert!(validate_input_string("language", "test").is_ok());
        assert!(validate_input_string("rowcount", "test").is_ok());
        assert!(validate_input_string("textsize", "test").is_ok());
    }
}
