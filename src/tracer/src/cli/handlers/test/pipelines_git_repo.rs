use crate::cli::handlers::test::pipeline::Pipeline;
use crate::utils::workdir::TRACER_WORK_DIR;
use anyhow::Result;
use git2::{AutotagOption, FetchOptions, Repository};
use std::path::PathBuf;

pub const TRACER_PIPELINES_REPO_URL: &str =
    "https://github.com/Tracer-Cloud/nextflow-test-pipelines.git";
pub const TRACER_PIPELINES_REPO_REL_PATH: &str = "nextflow-test-pipelines";

pub fn get_tracer_pipelines_repo_path() -> PathBuf {
    TRACER_WORK_DIR
        .resolve_canonical(TRACER_PIPELINES_REPO_REL_PATH)
        .unwrap()
}

pub fn get_tracer_pipeline_path(pipeline_name: &str) -> PathBuf {
    get_tracer_pipelines_repo_path()
        .join("pipelines/shared")
        .join(pipeline_name)
}

pub struct TracerPipelinesRepo {
    repo: Repository,
}

impl TracerPipelinesRepo {
    pub fn new() -> Result<Self> {
        if let Ok(repo) = Repository::discover(get_tracer_pipelines_repo_path()) {
            let pipelines_repo = TracerPipelinesRepo { repo };
            pipelines_repo.fetch()?;
            Ok(pipelines_repo)
        } else {
            let repo =
                Repository::clone(TRACER_PIPELINES_REPO_URL, get_tracer_pipelines_repo_path())?;
            let pipelines_repo = TracerPipelinesRepo { repo };
            pipelines_repo.checkout()?;
            Ok(pipelines_repo)
        }
    }

    fn fetch(&self) -> Result<()> {
        // Get the current branch
        let head = self.repo.head()?;
        let branch_name = head.shorthand().unwrap_or("main");

        // Find the remote
        let mut remote = self.repo.find_remote("origin")?;

        // Fetch the latest changes
        let mut fetch_options = FetchOptions::new();
        fetch_options.download_tags(AutotagOption::All);

        remote.fetch(&[branch_name], Some(&mut fetch_options), None)?;

        Ok(())
    }

    fn checkout(&self) -> Result<()> {
        // Find the main branch reference
        let main_branch = self
            .repo
            .find_branch("main", git2::BranchType::Local)
            .or_else(|_| {
                self.repo
                    .find_branch("origin/main", git2::BranchType::Remote)
            })?;

        // Get the commit that main points to
        let commit = main_branch.get().peel_to_commit()?;

        // Checkout the main branch
        self.repo.checkout_tree(&commit.into_object(), None)?;

        // Set HEAD to point to main
        self.repo.set_head("refs/heads/main")?;

        Ok(())
    }

    /// For now this returns a hard-coded list. After the re-org of the repo it will fetch the
    /// list from the repo itself.
    pub fn list_pipelines(&self) -> Vec<Pipeline> {
        // let mut path = self.repo.path();
        // if path.file_name().unwrap() == ".git" {
        //     path = path.parent().unwrap();
        // }
        let pipeline = Pipeline::tracer(get_tracer_pipeline_path("fastquorum")).unwrap();
        vec![pipeline]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::handlers::init::arguments::{PromptMode, TracerCliInitArgs};
    use crate::cli::handlers::test::arguments::TracerCliTestArgs;

    #[test]
    fn test_fastquorum_pipeline_resolution() {
        let args = TracerCliTestArgs {
            demo_pipeline_id: Some("fastquorum".to_string()),
            init_args: TracerCliInitArgs {
                interactive_prompts: PromptMode::Minimal,
                log_level: "info".into(),
                ..Default::default()
            },
        };

        let (_, pipeline) = args.resolve_test_arguments()
            .expect("failed to resolve pipeline");

        assert_eq!(pipeline.name(), "fastquorum");

        if let Pipeline::LocalPixi { path, .. } = &pipeline {
            assert_eq!(path, &get_tracer_pipeline_path("fastquorum"));
        } else {
            panic!("expected LocalPixi pipeline");
        }
    }

    #[test]
    fn test_get_tracer_pipeline_path() {
        let path = get_tracer_pipeline_path("fastquorum");
        assert!(path.to_string_lossy().contains("nextflow-test-pipelines"));
        assert!(path.to_string_lossy().contains("pipelines/shared/fastquorum"));
    }
}
