use super::Pipeline;
use crate::cli::handlers::INTERACTIVE_THEME;
use anyhow::{anyhow, Context, Result};
use dialoguer::Select;

const DEFAULT_PIPELINE: &str = "fastquorum";

/// Prompt user for pipeline selection with interactive UI
pub fn prompt_for_pipeline_selection(pipelines: &[Pipeline]) -> Result<String> {
    let mut names: Vec<&str> = pipelines.iter().map(|p| p.name()).collect();
    names.sort_unstable();

    let default_idx = names
        .iter()
        .position(|&name| name == DEFAULT_PIPELINE)
        .unwrap_or(0);

    let selection = Select::with_theme(&*INTERACTIVE_THEME)
        .with_prompt("Select pipeline to run")
        .items(&names)
        .default(default_idx)
        .interact()
        .context("pipeline selection failed")?;

    Ok(names[selection].to_string())
}

/// Get default pipeline or first available pipeline
pub fn get_default_or_first_pipeline(pipelines: &[Pipeline]) -> Result<String> {
    pipelines
        .iter()
        .find(|p| p.name() == DEFAULT_PIPELINE)
        .or_else(|| pipelines.first())
        .map(|p| p.name().to_string())
        .ok_or_else(|| anyhow!("no pipelines available"))
}
