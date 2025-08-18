mod execution;
mod pixi;
mod prompts;
mod selection;

use anyhow::{bail, Result};
use std::path::PathBuf;

pub enum Pipeline {
    LocalPixi {
        path: PathBuf,
        manifest: PathBuf,
        task: String,
    },
    LocalNextflow {
        path: PathBuf,
        args: Vec<String>,
    },
    GithubNextflow {
        repo: String,
        args: Vec<String>,
    },
    LocalTool {
        path: PathBuf,
        args: Vec<String>,
    },
}

impl Pipeline {
    pub fn tracer<P: Into<PathBuf>>(path: P) -> Result<Self> {
        Self::local_pixi(path, "pipeline")
    }

    pub fn local_pixi<P: Into<PathBuf>>(path: P, task: &str) -> Result<Self> {
        let path = path.into();
        let manifest = path.join("pixi.toml");
        Ok(Pipeline::LocalPixi {
            path,
            manifest,
            task: task.into(),
        })
    }

    pub fn name(&self) -> &str {
        match self {
            Pipeline::LocalPixi { path, .. } => path.file_name().unwrap().to_str().unwrap(),
            Pipeline::LocalNextflow { path, .. } => path.file_name().unwrap().to_str().unwrap(),
            Pipeline::LocalTool { path, .. } => path.file_name().unwrap().to_str().unwrap(),
            Pipeline::GithubNextflow { repo, .. } => repo,
        }
    }

    pub fn validate(&self) -> Result<()> {
        match self {
            Self::LocalPixi {
                path,
                manifest,
                task: _,
            } => {
                if !path.exists() {
                    bail!("Pipeline path does not exist: {path:?}");
                }
                if !manifest.exists() {
                    bail!("Pixi manifest file does not exist: {manifest:?}");
                }
                // TODO: look for task in manifest
            }
            Self::LocalNextflow { path, .. } => {
                if !path.exists() {
                    bail!("Pipeline path does not exist: {path:?}");
                }
            }
            Self::GithubNextflow { repo: _, .. } => {
                // TODO: validate repo
            }
            Self::LocalTool { path, .. } => {
                if which::which(path.file_name().expect("Invalid file name")).is_err() {
                    bail!("Tool path does not exist: {path:?}");
                }
            }
        }
        Ok(())
    }
}
