use anyhow::{bail, Result};
use std::path::PathBuf;

pub enum Pipeline {
    LocalPixi {
        path: PathBuf,
        manifest: PathBuf,
        task: String,
    },
    LocalCustom {
        path: PathBuf,
        args: Vec<String>,
    },
    LocalTool {
        path: PathBuf,
        args: Vec<String>,
    },
    GitHub {
        repo: String,
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
        if !manifest.exists() {
            bail!("Pixi manifest file does not exist: {manifest:?}");
        }
        Ok(Pipeline::LocalPixi {
            path,
            manifest,
            task: task.into(),
        })
    }

    pub fn name(&self) -> &str {
        match self {
            Pipeline::LocalPixi { path, .. } => path.file_name().unwrap().to_str().unwrap(),
            Pipeline::LocalCustom { path, .. } => path.file_name().unwrap().to_str().unwrap(),
            Pipeline::LocalTool { path, .. } => path.file_name().unwrap().to_str().unwrap(),
            Pipeline::GitHub { repo, .. } => repo,
        }
    }
}