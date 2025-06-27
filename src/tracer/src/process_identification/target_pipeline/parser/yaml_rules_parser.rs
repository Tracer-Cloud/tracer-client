use super::pipeline::{Dependencies, Job, Pipeline, Step, Subworkflow, Version};
use crate::utils::yaml::{Yaml, YamlExt, YamlVecLoader};
use anyhow::{anyhow, bail, Result};
use once_cell::sync::Lazy;
use std::path::Path;

static GLOBAL_DEPENDENCIES: Lazy<Dependencies> = Lazy::new(|| Dependencies::default());

pub fn load_yaml_pipelines<P: AsRef<Path>>(
    embedded_yaml: Option<&str>,
    fallback_paths: &[P],
) -> Vec<Pipeline> {
    YamlVecLoader {
        module: "PipelineManager",
        key: "pipelines",
        embedded_yaml,
        fallback_paths,
    }
    .load()
}

impl TryFrom<Yaml> for Pipeline {
    type Error = anyhow::Error;

    fn try_from(yaml: Yaml) -> Result<Self> {
        let id = yaml.required_string("id")?;
        let description = yaml.optional_string("description")?;
        let repo = yaml.optional_string("repo")?;
        let language = yaml.optional_string("language")?;
        let version = yaml.optional("version").map(|v| v.try_into()).transpose()?;
        let subworkflows = yaml
            .optional_vec("subworkflows")?
            .map(|v| {
                v.iter()
                    .map(|subworkflow| subworkflow.try_into())
                    .collect::<Result<Vec<_>>>()
            })
            .transpose()?;
        let jobs = yaml
            .optional_vec("jobs")?
            .map(|v| {
                v.iter()
                    .map(|job| job.try_into())
                    .collect::<Result<Vec<_>>>()
            })
            .transpose()?;
        let dependencies = if subworkflows.is_none() && jobs.is_none() {
            GLOBAL_DEPENDENCIES.clone()
        } else {
            Dependencies::new(subworkflows, jobs, Some(&GLOBAL_DEPENDENCIES))
        };
        let steps = yaml
            .optional_vec("steps")?
            .map(|v| {
                v.iter()
                    .map(|step| step.try_into())
                    .collect::<Result<Vec<_>>>()
            })
            .transpose()?;
        let optional_steps = yaml
            .optional_vec("optional_steps")?
            .map(|v| {
                v.iter()
                    .map(|step| step.try_into())
                    .collect::<Result<Vec<_>>>()
            })
            .transpose()?;
        Ok(Pipeline {
            id,
            description,
            repo,
            language,
            version,
            steps,
            optional_steps,
            dependencies,
        })
    }
}

impl TryFrom<&Yaml> for Version {
    type Error = anyhow::Error;

    fn try_from(yaml: &Yaml) -> Result<Self> {
        let min = yaml.optional_string("min")?;
        let max = yaml.optional_string("max")?;
        let exact = yaml.optional_string("exact")?;
        Ok(Version { min, max, exact })
    }
}

impl TryFrom<&Yaml> for Subworkflow {
    type Error = anyhow::Error;

    fn try_from(yaml: &Yaml) -> Result<Self> {
        let id = yaml.required_string("id")?;
        let description = yaml.optional_string("description")?;
        let steps = yaml
            .optional_vec("steps")?
            .map(|v| {
                v.iter()
                    .map(|step| step.try_into())
                    .collect::<Result<Vec<_>>>()
            })
            .transpose()?;
        let optional_steps = yaml
            .optional_vec("optional_steps")?
            .map(|v| {
                v.iter()
                    .map(|step| step.try_into())
                    .collect::<Result<Vec<_>>>()
            })
            .transpose()?;
        Ok(Subworkflow {
            id,
            description,
            steps,
            optional_steps,
        })
    }
}

impl TryFrom<&Yaml> for Job {
    type Error = anyhow::Error;

    fn try_from(yaml: &Yaml) -> Result<Self> {
        let id = yaml.required_string("id")?;
        let description = yaml.optional_string("description")?;
        let rules = yaml
            .required_vec("rules")?
            .iter()
            .map(|rule| {
                rule.as_str()
                    .ok_or(anyhow!("rule is not a string"))
                    .map(|s| s.to_string())
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(Job {
            id,
            description,
            rules,
        })
    }
}

impl TryFrom<&Yaml> for Step {
    type Error = anyhow::Error;

    fn try_from(yaml: &Yaml) -> Result<Self> {
        const STEP_TYPES: &[&str] = &["job", "optional_job", "subworkflow", "optional_subworkflow"];
        for step_type in STEP_TYPES {
            if let Some(id) = yaml.optional_string(step_type)? {
                return match *step_type {
                    "job" => Ok(Step::Job(id)),
                    "optional_job" => Ok(Step::OptionalJob(id)),
                    "subworkflow" => Ok(Step::Subworkflow(id)),
                    "optional_subworkflow" => Ok(Step::OptionalSubworkflow(id)),
                    _ => bail!("Unknown step type: {:?}", yaml),
                };
            }
        }

        const COND_TYPES: &[&str] = &["and", "or"];
        for cond_type in COND_TYPES {
            if let Some(conditions) = yaml.optional_vec(cond_type)? {
                let steps = conditions
                    .iter()
                    .map(|condition| condition.try_into())
                    .collect::<Result<Vec<_>>>()?;
                return match *cond_type {
                    "and" => Ok(Step::And(steps)),
                    "or" => Ok(Step::Or(steps)),
                    _ => bail!("Unknown condition type: {:?}", cond_type),
                };
            }
        }

        bail!("Invalid step: {:?}", yaml);
    }
}
