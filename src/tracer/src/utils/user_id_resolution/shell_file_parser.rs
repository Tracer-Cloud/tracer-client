use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;

/// Reads user ID from a specific shell configuration file
/// Parses lines looking for: export TRACER_USER_ID="value" or export TRACER_USER_ID=value
pub fn read_user_id_from_file(file_path: &PathBuf, export_pattern: &str) -> Result<Option<String>> {
    let content = fs::read_to_string(file_path)
        .with_context(|| format!("Failed to read file: {:?}", file_path))?;

    for line in content.lines() {
        let line = line.trim();
        if line.starts_with(export_pattern) {
            // Extract value from: export TRACER_USER_ID="value" or export TRACER_USER_ID=value
            let value_part = &line[export_pattern.len()..];
            let user_id = if value_part.starts_with('"') && value_part.ends_with('"') {
                // Remove quotes: "value" -> value
                value_part[1..value_part.len()-1].to_string()
            } else if value_part.starts_with('\'') && value_part.ends_with('\'') {
                // Remove single quotes: 'value' -> value
                value_part[1..value_part.len()-1].to_string()
            } else {
                // No quotes: value
                value_part.to_string()
            };
            
            if !user_id.trim().is_empty() {
                return Ok(Some(user_id));
            }
        }
    }

    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_read_user_id_from_file_with_double_quotes() -> Result<()> {
        let mut temp_file = NamedTempFile::new()?;
        writeln!(temp_file, "# Some comment")?;
        writeln!(temp_file, r#"export TRACER_USER_ID="quoted_user""#)?;
        writeln!(temp_file, "# Another comment")?;
        
        let result = read_user_id_from_file(&temp_file.path().to_path_buf(), "export TRACER_USER_ID=")?;
        assert_eq!(result, Some("quoted_user".to_string()));
        Ok(())
    }

    #[test]
    fn test_read_user_id_from_file_with_single_quotes() -> Result<()> {
        let mut temp_file = NamedTempFile::new()?;
        writeln!(temp_file, "export TRACER_USER_ID='single_quoted_user'")?;
        
        let result = read_user_id_from_file(&temp_file.path().to_path_buf(), "export TRACER_USER_ID=")?;
        assert_eq!(result, Some("single_quoted_user".to_string()));
        Ok(())
    }

    #[test]
    fn test_read_user_id_from_file_without_quotes() -> Result<()> {
        let mut temp_file = NamedTempFile::new()?;
        writeln!(temp_file, "export TRACER_USER_ID=unquoted_user")?;
        
        let result = read_user_id_from_file(&temp_file.path().to_path_buf(), "export TRACER_USER_ID=")?;
        assert_eq!(result, Some("unquoted_user".to_string()));
        Ok(())
    }

    #[test]
    fn test_read_user_id_from_file_not_found() -> Result<()> {
        let mut temp_file = NamedTempFile::new()?;
        writeln!(temp_file, "# No TRACER_USER_ID here")?;
        writeln!(temp_file, "export OTHER_VAR=value")?;
        
        let result = read_user_id_from_file(&temp_file.path().to_path_buf(), "export TRACER_USER_ID=")?;
        assert_eq!(result, None);
        Ok(())
    }

    #[test]
    fn test_read_user_id_from_file_empty_value() -> Result<()> {
        let mut temp_file = NamedTempFile::new()?;
        writeln!(temp_file, r#"export TRACER_USER_ID="""#)?;
        
        let result = read_user_id_from_file(&temp_file.path().to_path_buf(), "export TRACER_USER_ID=")?;
        assert_eq!(result, None);
        Ok(())
    }

    #[test]
    fn test_read_user_id_from_file_whitespace_value() -> Result<()> {
        let mut temp_file = NamedTempFile::new()?;
        writeln!(temp_file, r#"export TRACER_USER_ID="   ""#)?;
        
        let result = read_user_id_from_file(&temp_file.path().to_path_buf(), "export TRACER_USER_ID=")?;
        assert_eq!(result, None);
        Ok(())
    }

    #[test]
    fn test_read_user_id_from_file_multiple_lines() -> Result<()> {
        let mut temp_file = NamedTempFile::new()?;
        writeln!(temp_file, "# First comment")?;
        writeln!(temp_file, "export OTHER_VAR=other")?;
        writeln!(temp_file, r#"export TRACER_USER_ID="correct_user""#)?;
        writeln!(temp_file, "export ANOTHER_VAR=another")?;
        
        let result = read_user_id_from_file(&temp_file.path().to_path_buf(), "export TRACER_USER_ID=")?;
        assert_eq!(result, Some("correct_user".to_string()));
        Ok(())
    }

    #[test]
    fn test_read_user_id_from_file_first_match_wins() -> Result<()> {
        let mut temp_file = NamedTempFile::new()?;
        writeln!(temp_file, r#"export TRACER_USER_ID="first_user""#)?;
        writeln!(temp_file, r#"export TRACER_USER_ID="second_user""#)?;
        
        let result = read_user_id_from_file(&temp_file.path().to_path_buf(), "export TRACER_USER_ID=")?;
        assert_eq!(result, Some("first_user".to_string()));
        Ok(())
    }

    #[test]
    fn test_read_user_id_from_file_with_spaces_around_equals() -> Result<()> {
        let mut temp_file = NamedTempFile::new()?;
        writeln!(temp_file, "export TRACER_USER_ID = spaced_user")?;
        
        // This should not match because our pattern expects no spaces
        let result = read_user_id_from_file(&temp_file.path().to_path_buf(), "export TRACER_USER_ID=")?;
        assert_eq!(result, None);
        Ok(())
    }

    #[test]
    fn test_read_user_id_from_file_case_sensitive() -> Result<()> {
        let mut temp_file = NamedTempFile::new()?;
        writeln!(temp_file, "export tracer_user_id=lowercase")?;
        
        // Should not match because case is different
        let result = read_user_id_from_file(&temp_file.path().to_path_buf(), "export TRACER_USER_ID=")?;
        assert_eq!(result, None);
        Ok(())
    }
}
