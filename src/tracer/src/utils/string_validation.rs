use std::str::FromStr;
use uuid::Uuid;

/**
 * This function checks if a string is a valid UUID
 */
pub fn is_valid_uuid(input_string: &str) -> bool {
    Uuid::from_str(input_string).is_ok()
}
