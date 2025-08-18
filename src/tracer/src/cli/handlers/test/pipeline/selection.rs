use super::prompts::{get_default_or_first_pipeline, prompt_for_pipeline_selection};
use super::Pipeline;
use crate::cli::handlers::init::arguments::PromptMode;
use crate::cli::handlers::test::pipelines_git_repo::TracerPipelinesRepo;
use anyhow::{anyhow, Result};

impl Pipeline {
    /// Pure function for pipeline selection
    pub fn select_test_pipeline(
        demo_pipeline_id: Option<String>,
        interactive_prompts: PromptMode,
    ) -> Result<Pipeline> {
        let pipelines = sync_pipelines()?;
        let pipeline_name =
            select_pipeline_name(demo_pipeline_id, interactive_prompts, &pipelines)?;
        find_and_validate_pipeline(pipelines, &pipeline_name)
    }
}

// Pipeline selection helper functions

/// Sync pipelines from repository
fn sync_pipelines() -> Result<Vec<Pipeline>> {
    Ok(TracerPipelinesRepo::new()?.list_pipelines())
}

/// Select pipeline name based on user input or interactive prompt
fn select_pipeline_name(
    demo_pipeline_id: Option<String>,
    interactive_prompts: PromptMode,
    pipelines: &[Pipeline],
) -> Result<String> {
    match demo_pipeline_id {
        Some(name) => validate_pipeline_exists(pipelines, &name),
        None => choose_pipeline_with_interactive_prompt(interactive_prompts, pipelines),
    }
}

/// Validate that a pipeline with the given name exists
fn validate_pipeline_exists(pipelines: &[Pipeline], name: &str) -> Result<String> {
    pipelines
        .iter()
        .find(|p| p.name() == name)
        .map(|_| name.to_string())
        .ok_or_else(|| anyhow!("pipeline '{}' not found", name))
}

/// Choose pipeline with interactive prompt or default
fn choose_pipeline_with_interactive_prompt(
    interactive_prompts: PromptMode,
    pipelines: &[Pipeline],
) -> Result<String> {
    let is_interactive = interactive_prompts != PromptMode::None;

    if is_interactive && pipelines.len() > 1 {
        prompt_for_pipeline_selection(pipelines)
    } else {
        get_default_or_first_pipeline(pipelines)
    }
}

/// Find and validate pipeline by name
fn find_and_validate_pipeline(pipelines: Vec<Pipeline>, name: &str) -> Result<Pipeline> {
    pipelines
        .into_iter()
        .find(|p| p.name() == name)
        .ok_or_else(|| anyhow!("pipeline '{}' not found", name))
        .and_then(|p| p.validate().map(|_| p))
}
