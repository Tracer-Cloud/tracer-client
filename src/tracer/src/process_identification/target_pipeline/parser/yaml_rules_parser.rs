use super::pipeline::{Dependencies, Job, Pipeline, Step, Subworkflow, Version};
use crate::utils::yaml::{Yaml, YamlExt, YamlFile};
use anyhow::{anyhow, bail, Result};
use std::sync::LazyLock;
use tracing::error;

static GLOBAL_DEPENDENCIES: LazyLock<Dependencies> = LazyLock::new(Dependencies::default);

pub fn load_pipelines_from_yamls(yaml_files: &[YamlFile]) -> Vec<Pipeline> {
    yaml_files
        .iter()
        .flat_map(|yaml_file| match yaml_file.load::<Pipeline>("pipelines") {
            Ok(pipelines) => pipelines,
            Err(e) => {
                error!("Error loading yaml file {:?}: {}", yaml_file, e);
                vec![]
            }
        })
        .collect()
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
        let optional_rules = yaml
            .optional_vec("optional_rules")?
            .map(|rules| {
                rules
                    .iter()
                    .map(|rule| {
                        rule.as_str()
                            .ok_or(anyhow!("rule is not a string"))
                            .map(|s| s.to_string())
                    })
                    .collect::<Result<Vec<_>>>()
            })
            .transpose()?;
        Ok(Job {
            id,
            description,
            rules,
            optional_rules,
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

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions_sorted::assert_eq;

    const PIPELINE_YAML_PATH: &[YamlFile] = &[YamlFile::StaticPath(
        "src/process_identification/target_pipeline/yml_rules/tracer.pipelines.yml",
    )];

    #[test]
    fn test_load_pipelines_from_yaml_from_file() {
        let pipelines = load_pipelines_from_yamls(PIPELINE_YAML_PATH);

        // Should load exactly one pipeline
        assert_eq!(pipelines.len(), 1);
        let pipeline = &pipelines[0];

        // Test basic pipeline properties
        assert_eq!(pipeline.id, "nf-core/rnaseq");
        assert_eq!(
            pipeline.description,
            Some("RNA sequencing analysis pipeline for gene/isoform quantification and extensive quality control.".to_string())
        );
        assert_eq!(
            pipeline.repo,
            Some("https://github.com/nf-core/rnaseq".to_string())
        );
        assert_eq!(pipeline.language, Some("nextflow".to_string()));

        // Test version
        assert!(pipeline.version.is_some());
        let version = pipeline.version.as_ref().unwrap();
        assert_eq!(version.min, Some("3.19.0".to_string()));
        assert_eq!(version.max, None);
        assert_eq!(version.exact, None);

        // Test subworkflows
        assert!(pipeline
            .dependencies
            .subworkflows
            .contains_key("PREPARE_GENOME"));
        let subworkflow = pipeline
            .dependencies
            .subworkflows
            .get("PREPARE_GENOME")
            .unwrap();
        assert_eq!(subworkflow.id, "PREPARE_GENOME");
        assert_eq!(
            subworkflow.description,
            Some("Create genome indexes for RNA-seq analysis.".to_string())
        );
        assert!(subworkflow.steps.is_some());
        let steps = subworkflow.steps.as_ref().unwrap();
        assert_eq!(steps.len(), 2);
        match &steps[0] {
            Step::Or(or_steps) => {
                assert_eq!(or_steps.len(), 2);
                match &or_steps[0] {
                    Step::Job(job_id) => assert_eq!(job_id, "GUNZIP_GTF"),
                    _ => panic!("Expected Job step"),
                }
                match &or_steps[1] {
                    Step::And(and_steps) => {
                        assert_eq!(and_steps.len(), 2);
                        match &and_steps[0] {
                            Step::OptionalJob(job_id) => assert_eq!(job_id, "GUNZIP_GFF"),
                            _ => panic!("Expected OptionalJob step"),
                        }
                        match &and_steps[1] {
                            Step::Job(job_id) => assert_eq!(job_id, "GFFREAD"),
                            _ => panic!("Expected Job step"),
                        }
                    }
                    _ => panic!("Expected And step"),
                }
            }
            _ => panic!("Expected Or step"),
        }

        // Test jobs
        assert!(pipeline.dependencies.jobs.contains_key("GUNZIP_GTF"));
        assert!(pipeline.dependencies.jobs.contains_key("GUNZIP_GFF"));
        assert!(pipeline.dependencies.jobs.contains_key("GFFREAD"));
        let gunzip_gtf = pipeline.dependencies.jobs.get("GUNZIP_GTF").unwrap();
        assert_eq!(gunzip_gtf.id, "GUNZIP_GTF");
        assert_eq!(
            gunzip_gtf.description,
            Some("Unzip the GTF file.".to_string())
        );
        assert_eq!(gunzip_gtf.rules, vec!["gunzip_gtf"]);
        let gunzip_gff = pipeline.dependencies.jobs.get("GUNZIP_GFF").unwrap();
        assert_eq!(gunzip_gff.id, "GUNZIP_GFF");
        assert_eq!(
            gunzip_gff.description,
            Some("Unzip the GFF file.".to_string())
        );
        assert_eq!(gunzip_gff.rules, vec!["gunzip_gff"]);
        let gffread = pipeline.dependencies.jobs.get("GFFREAD").unwrap();
        assert_eq!(gffread.id, "GFFREAD");
        assert_eq!(gffread.description, Some("Read the GFF file.".to_string()));
        assert_eq!(gffread.rules, vec!["gffread"]);

        // Test main pipeline steps
        assert!(pipeline.steps.is_some());
        let steps = pipeline.steps.as_ref().unwrap();
        assert_eq!(steps.len(), 1);
        match &steps[0] {
            Step::Subworkflow(subworkflow_id) => assert_eq!(subworkflow_id, "PREPARE_GENOME"),
            _ => panic!("Expected Subworkflow step"),
        }
        assert!(pipeline.optional_steps.is_none());
    }

    #[test]
    fn test_pipeline_subworkflows() {
        let pipelines = load_pipelines_from_yamls(PIPELINE_YAML_PATH);
        let pipeline = &pipelines[0];

        // Test subworkflows
        assert!(pipeline
            .dependencies
            .subworkflows
            .contains_key("PREPARE_GENOME"));

        let subworkflow = pipeline
            .dependencies
            .subworkflows
            .get("PREPARE_GENOME")
            .unwrap();
        assert_eq!(subworkflow.id, "PREPARE_GENOME");
        assert_eq!(
            subworkflow.description,
            Some("Create genome indexes for RNA-seq analysis.".to_string())
        );

        // Test subworkflow steps
        assert!(subworkflow.steps.is_some());
        let steps = subworkflow.steps.as_ref().unwrap();
        assert_eq!(steps.len(), 2);

        // Test the OR step
        match &steps[0] {
            Step::Or(or_steps) => {
                assert_eq!(or_steps.len(), 2);

                // First step should be GUNZIP_GTF job
                match &or_steps[0] {
                    Step::Job(job_id) => assert_eq!(job_id, "GUNZIP_GTF"),
                    _ => panic!("Expected Job step"),
                }

                // Second step should be AND with optional_job and job
                match &or_steps[1] {
                    Step::And(and_steps) => {
                        assert_eq!(and_steps.len(), 2);

                        match &and_steps[0] {
                            Step::OptionalJob(job_id) => assert_eq!(job_id, "GUNZIP_GFF"),
                            _ => panic!("Expected OptionalJob step"),
                        }

                        match &and_steps[1] {
                            Step::Job(job_id) => assert_eq!(job_id, "GFFREAD"),
                            _ => panic!("Expected Job step"),
                        }
                    }
                    _ => panic!("Expected And step"),
                }
            }
            _ => panic!("Expected Or step"),
        }
    }

    #[test]
    fn test_pipeline_jobs() {
        let pipelines = load_pipelines_from_yamls(PIPELINE_YAML_PATH);
        let pipeline = &pipelines[0];

        // Test jobs
        assert!(pipeline.dependencies.jobs.contains_key("GUNZIP_GTF"));
        assert!(pipeline.dependencies.jobs.contains_key("GUNZIP_GFF"));
        assert!(pipeline.dependencies.jobs.contains_key("GFFREAD"));

        // Test GUNZIP_GTF job
        let gunzip_gtf = pipeline.dependencies.jobs.get("GUNZIP_GTF").unwrap();
        assert_eq!(gunzip_gtf.id, "GUNZIP_GTF");
        assert_eq!(
            gunzip_gtf.description,
            Some("Unzip the GTF file.".to_string())
        );
        assert_eq!(gunzip_gtf.rules, vec!["gunzip_gtf"]);

        // Test GUNZIP_GFF job
        let gunzip_gff = pipeline.dependencies.jobs.get("GUNZIP_GFF").unwrap();
        assert_eq!(gunzip_gff.id, "GUNZIP_GFF");
        assert_eq!(
            gunzip_gff.description,
            Some("Unzip the GFF file.".to_string())
        );
        assert_eq!(gunzip_gff.rules, vec!["gunzip_gff"]);

        // Test GFFREAD job
        let gffread = pipeline.dependencies.jobs.get("GFFREAD").unwrap();
        assert_eq!(gffread.id, "GFFREAD");
        assert_eq!(gffread.description, Some("Read the GFF file.".to_string()));
        assert_eq!(gffread.rules, vec!["gffread"]);
    }

    #[test]
    fn test_pipeline_steps() {
        let pipelines = load_pipelines_from_yamls(PIPELINE_YAML_PATH);
        let pipeline = &pipelines[0];

        // Test main pipeline steps
        assert!(pipeline.steps.is_some());
        let steps = pipeline.steps.as_ref().unwrap();
        assert_eq!(steps.len(), 1);

        // Test the subworkflow step
        match &steps[0] {
            Step::Subworkflow(subworkflow_id) => {
                assert_eq!(subworkflow_id, "PREPARE_GENOME");
            }
            _ => panic!("Expected Subworkflow step"),
        }

        // Test optional_steps (should be None for this pipeline)
        assert!(pipeline.optional_steps.is_none());
    }

    #[test]
    fn test_pipeline_dependencies_structure() {
        let pipelines = load_pipelines_from_yamls(PIPELINE_YAML_PATH);
        let pipeline = &pipelines[0];

        // Test that dependencies are properly structured
        assert_eq!(pipeline.dependencies.subworkflows.len(), 1);
        assert_eq!(pipeline.dependencies.jobs.len(), 5);

        // Test that parent dependencies are set (should be GLOBAL_DEPENDENCIES)
        assert!(pipeline.dependencies.parent.is_some());
    }

    #[test]
    fn test_load_pipelines_from_yaml_with_embedded_yaml() {
        let embedded_yaml = r#"
pipelines:
  - id: test-pipeline
    description: Test pipeline for unit testing
    language: test
    version:
      min: "1.0.0"
      max: "2.0.0"
    jobs:
      - id: TEST_JOB
        description: A test job
        rules:
          - test_rule
      - id: TEST_JOB_2
        description: Another test job
        rules:
          - test_rule_2
    subworkflows:
      - id: TEST_SUBWORKFLOW
        description: A test subworkflow
        steps:
          - job: TEST_JOB
    steps:
      - subworkflow: TEST_SUBWORKFLOW
      - job: TEST_JOB_2
"#;

        let pipelines = load_pipelines_from_yamls(&[YamlFile::Embedded(embedded_yaml)]);

        // Should load the embedded pipeline
        assert_eq!(pipelines.len(), 1);

        let pipeline = &pipelines[0];
        assert_eq!(pipeline.id, "test-pipeline");
        assert_eq!(
            pipeline.description,
            Some("Test pipeline for unit testing".to_string())
        );
        assert_eq!(pipeline.language, Some("test".to_string()));

        // Test version
        assert!(pipeline.version.is_some());
        let version = pipeline.version.as_ref().unwrap();
        assert_eq!(version.min, Some("1.0.0".to_string()));
        assert_eq!(version.max, Some("2.0.0".to_string()));
        assert_eq!(version.exact, None);

        // Test jobs
        assert!(pipeline.dependencies.jobs.contains_key("TEST_JOB"));
        assert!(pipeline.dependencies.jobs.contains_key("TEST_JOB_2"));

        let job1 = pipeline.dependencies.jobs.get("TEST_JOB").unwrap();
        assert_eq!(job1.id, "TEST_JOB");
        assert_eq!(job1.description, Some("A test job".to_string()));
        assert_eq!(job1.rules, vec!["test_rule"]);

        let job2 = pipeline.dependencies.jobs.get("TEST_JOB_2").unwrap();
        assert_eq!(job2.id, "TEST_JOB_2");
        assert_eq!(job2.description, Some("Another test job".to_string()));
        assert_eq!(job2.rules, vec!["test_rule_2"]);

        // Test subworkflows
        assert!(pipeline
            .dependencies
            .subworkflows
            .contains_key("TEST_SUBWORKFLOW"));
        let subworkflow = pipeline
            .dependencies
            .subworkflows
            .get("TEST_SUBWORKFLOW")
            .unwrap();
        assert_eq!(subworkflow.id, "TEST_SUBWORKFLOW");
        assert_eq!(
            subworkflow.description,
            Some("A test subworkflow".to_string())
        );

        // Test subworkflow steps
        assert!(subworkflow.steps.is_some());
        let subworkflow_steps = subworkflow.steps.as_ref().unwrap();
        assert_eq!(subworkflow_steps.len(), 1);
        match &subworkflow_steps[0] {
            Step::Job(job_id) => assert_eq!(job_id, "TEST_JOB"),
            _ => panic!("Expected Job step"),
        }

        // Test main pipeline steps
        assert!(pipeline.steps.is_some());
        let steps = pipeline.steps.as_ref().unwrap();
        assert_eq!(steps.len(), 2);

        match &steps[0] {
            Step::Subworkflow(subworkflow_id) => assert_eq!(subworkflow_id, "TEST_SUBWORKFLOW"),
            _ => panic!("Expected Subworkflow step"),
        }

        match &steps[1] {
            Step::Job(job_id) => assert_eq!(job_id, "TEST_JOB_2"),
            _ => panic!("Expected Job step"),
        }
    }

    #[test]
    fn test_load_pipelines_from_yaml_with_complex_steps() {
        let embedded_yaml = r#"
pipelines:
  - id: complex-pipeline
    description: Pipeline with complex step structures
    jobs:
      - id: JOB1
        description: First job
        rules:
          - rule1
      - id: JOB2
        description: Second job
        rules:
          - rule2
      - id: JOB3
        description: Third job
        rules:
          - rule3
    steps:
      - or:
          - job: JOB1
          - and:
              - optional_job: JOB2
              - job: JOB3
"#;

        let pipelines = load_pipelines_from_yamls(&[YamlFile::Embedded(embedded_yaml)]);

        assert_eq!(pipelines.len(), 1);
        let pipeline = &pipelines[0];

        // Test complex step structure
        assert!(pipeline.steps.is_some());
        let steps = pipeline.steps.as_ref().unwrap();
        assert_eq!(steps.len(), 1);

        // Test the OR step
        match &steps[0] {
            Step::Or(or_steps) => {
                assert_eq!(or_steps.len(), 2);

                // First step should be JOB1
                match &or_steps[0] {
                    Step::Job(job_id) => assert_eq!(job_id, "JOB1"),
                    _ => panic!("Expected Job step"),
                }

                // Second step should be AND with optional_job and job
                match &or_steps[1] {
                    Step::And(and_steps) => {
                        assert_eq!(and_steps.len(), 2);

                        match &and_steps[0] {
                            Step::OptionalJob(job_id) => assert_eq!(job_id, "JOB2"),
                            _ => panic!("Expected OptionalJob step"),
                        }

                        match &and_steps[1] {
                            Step::Job(job_id) => assert_eq!(job_id, "JOB3"),
                            _ => panic!("Expected Job step"),
                        }
                    }
                    _ => panic!("Expected And step"),
                }
            }
            _ => panic!("Expected Or step"),
        }
    }
}
