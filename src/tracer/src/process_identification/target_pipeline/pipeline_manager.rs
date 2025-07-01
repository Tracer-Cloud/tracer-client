use crate::process_identification::target_pipeline::parser::pipeline::{
    Dependencies, Job, Pipeline, Step,
};
use crate::process_identification::target_pipeline::parser::yaml_rules_parser::load_pipelines_from_yamls;
use crate::process_identification::target_process::target::Target;
use crate::utils::yaml::YamlFile;
use multi_index_map::MultiIndexMap;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use tracer_ebpf::ebpf_trigger::ProcessStartTrigger;
use tracing::trace;

pub const JOB_SCORE_THRESHOLD: f64 = 0.9;

/// A job that is matched to a set of processes that have been started.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JobMatch {
    /// The ID of the job that was matched - this is used for labeling the process group in the UI.
    pub id: String,
    /// The description of the job that was matched.
    pub description: Option<String>,
    /// The PIDs that have been matched to this job.
    pub pids: Vec<usize>,
    /// The score of the job that was matched.
    pub score: f64,
}

impl std::fmt::Display for JobMatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "JobMatch(id: {}, description: {:?}, pids: {:?}, score: {})",
            self.id, self.description, self.pids, self.score
        )
    }
}

pub struct TargetPipelineManager {
    _pipelines: Vec<Pipeline>,
    jobs: Jobs,
    job_pids: MultiIndexJobPidMap,
    pid_to_process: HashMap<usize, ProcessRule>,
}

impl TargetPipelineManager {
    pub fn new(rule_files: &[YamlFile], _targets: &Vec<Target>) -> Self {
        let pipelines = load_pipelines_from_yamls(rule_files);
        let mut jobs = Jobs::default();
        pipelines.iter().for_each(|pipeline| {
            let dependencies = &pipeline.dependencies;
            if let Some(steps) = &pipeline.steps {
                jobs.add_steps(steps, dependencies, false);
            }
            if let Some(steps) = &pipeline.optional_steps {
                jobs.add_steps(steps, dependencies, true);
            }
        });
        Self {
            _pipelines: pipelines,
            jobs,
            job_pids: MultiIndexJobPidMap::default(),
            pid_to_process: HashMap::new(),
        }
    }

    pub fn register_process(
        &mut self,
        process: &ProcessStartTrigger,
        matched_target: Option<&String>,
    ) -> Option<JobMatch> {
        // TODO: this can lead to false-negatives if PIDs are reused.
        if self.pid_to_process.contains_key(&process.pid) {
            trace!("PID {} is already registered", process.pid);
            return None;
        }

        let (rule, matched) = if let Some(display_name) = matched_target {
            (display_name, true)
        } else {
            (&process.comm, false)
        };
        if let Some(jobs) = self.jobs.get_jobs_with(rule) {
            // add the PID to the set we're tracking
            self.pid_to_process.insert(
                process.pid,
                ProcessRule {
                    name: rule.clone(),
                    matched,
                },
            );
            // find any jobs that exceed the score threshold after adding the rule
            let mut matched_jobs = jobs
                .iter()
                .filter_map(|job| {
                    // TODO: should we check that the rule associated with the PID is not one
                    // that has already been recognized? Sometimes, the same command will be run
                    // multiple times in the same job. We could require a separate rule entry in
                    // the job definition for each time the command is run, or have a way to
                    // specify the cardinality of the rule within the job.

                    // add the PID to the list for candidate job
                    self.job_pids.insert(JobPid {
                        job_id: job.id.clone(),
                        pid: process.pid,
                    });
                    let job_pids = self.job_pids.get_by_job_id(&job.id);
                    // For now score is just the fraction of rules that have been observed.
                    // TODO: weight score based on whether the rule is optional or not.
                    let score = job_pids.len() as f64
                        / (job.rules.len()
                            + job.optional_rules.as_ref().map(|v| v.len()).unwrap_or(0))
                            as f64;
                    if score > JOB_SCORE_THRESHOLD {
                        let pids: Vec<usize> = job_pids.iter().map(|job_pid| job_pid.pid).collect();
                        Some((job, pids, score))
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();
            if matched_jobs.is_empty() {
                return None;
            }
            // if there are multiple matches, pick the one with the highest score
            if matched_jobs.len() > 1 {
                matched_jobs.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap());
            }
            let (best_match, pids, score) = matched_jobs.pop().unwrap();
            if score >= 1.0 {
                // If the match is perfect (i.e. all rules have been matched to processes) then:
                // 1) remove the job so we don't update/match it again
                self.job_pids.remove_by_job_id(&best_match.id);
                // 2) remove the PIDs associated with the best match from any other candidate jobs
                for pid in pids.iter() {
                    self.job_pids.remove_by_pid(pid);
                }
            }
            return Some(JobMatch {
                id: best_match.id.clone(),
                description: best_match.description.clone(),
                pids,
                score,
            });
        }

        None
    }
}

impl Default for TargetPipelineManager {
    fn default() -> Self {
        const RULE_FILES: &[YamlFile] = &[YamlFile::Embedded(include_str!(
            "yml_rules/tracer.pipelines.yml"
        ))];
        Self::new(RULE_FILES, &Vec::new())
    }
}

impl Default for TargetPipelineManager {
    fn default() -> Self {
        Self::new(&[], &[])
    }
}

/// A bidirectional many-to-many mapping between jobs and PIDs.
#[derive(MultiIndexMap, Debug)]
struct JobPid {
    #[multi_index(hashed_non_unique)]
    job_id: String,
    #[multi_index(hashed_non_unique)]
    pid: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ProcessRule {
    name: String,
    matched: bool,
}

#[derive(Debug, Clone, Default)]
struct Jobs {
    jobs: HashMap<String, Job>,
    rule_to_job: HashMap<String, HashSet<String>>,
}

impl Jobs {
    fn add_steps(&mut self, steps: &Vec<Step>, dependencies: &Dependencies, optional: bool) {
        for step in steps {
            match step {
                Step::Job(id) => self.add_job(id, dependencies, optional),
                Step::OptionalJob(id) => self.add_job(id, dependencies, true),
                Step::Subworkflow(id) => self.add_subworkflow(id, dependencies, optional),
                Step::OptionalSubworkflow(id) => self.add_subworkflow(id, dependencies, true),
                Step::And(steps) => self.add_steps(steps, dependencies, optional),
                Step::Or(steps) => self.add_steps(steps, dependencies, optional),
            }
        }
    }

    fn add_job(&mut self, id: &String, dependencies: &Dependencies, _optional: bool) {
        if let Some(job) = dependencies.get_job(id) {
            if !self.jobs.contains_key(id) {
                self.jobs.insert(id.clone(), job.clone());
            }
            for rule in &job.rules {
                if let Some(jobs) = self.rule_to_job.get_mut(rule) {
                    jobs.insert(job.id.clone());
                } else {
                    self.rule_to_job
                        .insert(rule.clone(), HashSet::from([job.id.clone()]));
                }
            }
            if let Some(optional_rules) = job.optional_rules.as_ref() {
                for rule in optional_rules {
                    if let Some(jobs) = self.rule_to_job.get_mut(rule) {
                        jobs.insert(job.id.clone());
                    } else {
                        self.rule_to_job
                            .insert(rule.clone(), HashSet::from([job.id.clone()]));
                    }
                }
            }
        }
    }

    fn add_subworkflow(&mut self, id: &String, dependencies: &Dependencies, optional: bool) {
        if let Some(subworkflow) = dependencies.get_subworkflow(id) {
            if let Some(steps) = &subworkflow.steps {
                self.add_steps(steps, dependencies, optional);
            }
            if let Some(steps) = &subworkflow.optional_steps {
                self.add_steps(steps, dependencies, true);
            }
        }
    }

    fn get_jobs_with(&self, rule: &str) -> Option<HashSet<&Job>> {
        self.rule_to_job
            .get(rule)
            .map(|jobs| jobs.iter().map(|job| self.jobs.get(job).unwrap()).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::process_identification::target_process::target::Target;
    use crate::process_identification::target_process::target_match::MatchType;
    use pretty_assertions_sorted::assert_eq;
    use rstest::*;
    use tracer_ebpf::ebpf_trigger::ProcessStartTrigger;

    const PIPELINE_YAML_PATH: &[YamlFile] = &[YamlFile::StaticPath(
        "src/process_identification/target_pipeline/yml_rules/tracer.pipelines.yml",
    )];

    impl TargetPipelineManager {
        fn num_unmatched_jobs(&self) -> usize {
            self.job_pids
                .iter()
                .map(|(_, job_pid)| &job_pid.job_id)
                .collect::<HashSet<_>>()
                .len()
        }

        fn num_unmatched_pids(&self) -> usize {
            self.job_pids
                .iter()
                .map(|(_, job_pid)| &job_pid.pid)
                .collect::<HashSet<_>>()
                .len()
        }
    }

    /// Fixture that creates test targets for the pipeline rules
    #[fixture]
    #[once]
    fn test_targets() -> Vec<Target> {
        vec![
            Target {
                match_type: MatchType::And(Vec::from([
                    MatchType::ProcessNameIs("gzip".to_string()),
                    MatchType::FirstArgIs("-cd".to_string()),
                    MatchType::CommandContains(".gtf.gz".to_string()),
                ])),
                display_name: "gunzip_gtf".to_string(),
            },
            Target {
                match_type: MatchType::And(Vec::from([
                    MatchType::ProcessNameIs("gzip".to_string()),
                    MatchType::FirstArgIs("-cd".to_string()),
                    MatchType::CommandContains(".gff.gz".to_string()),
                ])),
                display_name: "gunzip_gff".to_string(),
            },
            Target {
                match_type: MatchType::ProcessNameIs("gffread".to_string()),
                display_name: "gffread".to_string(),
            },
            Target {
                match_type: MatchType::ProcessNameIs("bbsplit.sh".to_string()),
                display_name: "bbsplit".to_string(),
            },
            Target {
                match_type: MatchType::ProcessNameIs("jshell".to_string()),
                display_name: "jshell".to_string(),
            },
            Target {
                match_type: MatchType::And(Vec::from([
                    MatchType::ProcessNameIs("samtools".to_string()),
                    MatchType::FirstArgIs("faidx".to_string()),
                ])),
                display_name: "samtools faidx".to_string(),
            },
            Target {
                match_type: MatchType::And(Vec::from([
                    MatchType::ProcessNameIs("STAR".to_string()),
                    MatchType::CommandContains("--runMode genomeGenerate".to_string()),
                ])),
                display_name: "STAR index".to_string(),
            },
        ]
    }

    /// Fixture that creates a TargetPipelineManager with default pipeline rules
    #[fixture]
    fn pipeline_manager(test_targets: &Vec<Target>) -> TargetPipelineManager {
        TargetPipelineManager::new(PIPELINE_YAML_PATH, test_targets)
    }

    /// Helper function to create a process start trigger
    fn create_process_trigger(comm: &str, pid: usize) -> ProcessStartTrigger {
        ProcessStartTrigger::from_command_string(pid, 1, comm)
    }

    /// Helper function to find a target by display name
    fn find_target_by_display_name<'a>(
        targets: &'a [Target],
        display_name: &str,
    ) -> Option<&'a Target> {
        targets
            .iter()
            .find(|target| target.display_name == display_name)
    }

    #[rstest]
    fn test_register_single_process_no_match(mut pipeline_manager: TargetPipelineManager) {
        // Register a process that doesn't match any pipeline rules
        let process = create_process_trigger("unrelated_process", 1001);
        let result = pipeline_manager.register_process(&process, None);
        // Should return None since no pipeline rules match
        assert_eq!(result, None);
    }

    #[rstest]
    fn test_register_gunzip_gtf_process(
        mut pipeline_manager: TargetPipelineManager,
        test_targets: &Vec<Target>,
    ) {
        // Register a gunzip process that matches the gunzip_gtf rule
        let process = create_process_trigger("gzip -cd foo.gtf.gz", 1001);
        let target = find_target_by_display_name(&test_targets, "gunzip_gtf").unwrap();
        let result = pipeline_manager.register_process(&process, Some(&target.display_name));

        assert_eq!(
            result,
            Some(JobMatch {
                id: "GUNZIP_GTF".to_string(),
                description: Some("Unzip the GTF file.".to_string()),
                pids: vec![1001],
                score: 1.0,
            })
        );
        assert_eq!(pipeline_manager.num_unmatched_jobs(), 0);
        assert_eq!(pipeline_manager.num_unmatched_pids(), 0);
    }

    #[rstest]
    fn test_register_all_processes_for_bbmap_bbsplit(
        pipeline_manager: TargetPipelineManager,
        test_targets: &Vec<Target>,
    ) {
        let mut manager = pipeline_manager;

        // Register jshell process
        let jshell_process = create_process_trigger("jshell", 1002);
        let jshell_target = find_target_by_display_name(&test_targets, "jshell").unwrap();
        let result1 = manager.register_process(&jshell_process, Some(&jshell_target.display_name));

        // Should return None since we need more processes
        assert_eq!(result1, None);

        // Register bbsplit process
        let bbsplit_process = create_process_trigger("bbsplit.sh", 1001);
        let bbsplit_target = find_target_by_display_name(&test_targets, "bbsplit").unwrap();
        let result2 =
            manager.register_process(&bbsplit_process, Some(&bbsplit_target.display_name));

        // Should return a match for the BBMAP_BBSPLIT job since it has score 1.0 (both rules matched)
        assert!(result2.is_some());
        let job_match = result2.unwrap();
        assert_eq!(job_match.id, "BBMAP_BBSPLIT");
        assert_eq!(
            job_match.description,
            Some("Split the FASTQ file into smaller chunks.".to_string())
        );
        assert_eq!(job_match.score, 1.0);
        assert_eq!(job_match.pids, vec![1002, 1001]);
    }

    #[rstest]
    fn test_register_all_processes_for_star_preparegenome(
        pipeline_manager: TargetPipelineManager,
        test_targets: &Vec<Target>,
    ) {
        let mut manager = pipeline_manager;

        // Register (optional) samtools process
        let samtools_process = create_process_trigger("samtools faidx", 1002);
        let samtools_target = find_target_by_display_name(&test_targets, "samtools faidx").unwrap();
        let result1 =
            manager.register_process(&samtools_process, Some(&samtools_target.display_name));

        // Should return None since we need more processes
        assert_eq!(result1, None);

        // Register STAR process
        let star_process = create_process_trigger("STAR --runMode genomeGenerate", 1001);
        let star_target = find_target_by_display_name(&test_targets, "STAR index").unwrap();
        let result2 = manager.register_process(&star_process, Some(&star_target.display_name));

        // Should return a match for the STAR_GENOMEGENERATE job since it has score 1.0 (both rules matched)
        assert!(result2.is_some());
        let job_match = result2.unwrap();
        assert_eq!(job_match.id, "STAR_GENOMEGENERATE");
        assert_eq!(
            job_match.description,
            Some("Generate the genome index for STAR.".to_string())
        );
        assert_eq!(job_match.score, 1.0);
        assert_eq!(job_match.pids, vec![1002, 1001]);
    }

    #[rstest]
    fn test_duplicate_pid_registration(
        mut pipeline_manager: TargetPipelineManager,
        test_targets: &Vec<Target>,
    ) {
        // Register a gunzip process that matches the gunzip_gtf rule
        let process = create_process_trigger("gzip -cd foo.gtf.gz", 1001);
        let target = find_target_by_display_name(&test_targets, "gunzip_gtf").unwrap();
        let result1 = pipeline_manager.register_process(&process, Some(&target.display_name));

        assert_eq!(
            result1,
            Some(JobMatch {
                id: "GUNZIP_GTF".to_string(),
                description: Some("Unzip the GTF file.".to_string()),
                pids: vec![1001],
                score: 1.0,
            })
        );
        assert_eq!(pipeline_manager.num_unmatched_jobs(), 0);
        assert_eq!(pipeline_manager.num_unmatched_pids(), 0);

        // Register the same process again - should be ignored
        let result2 = pipeline_manager.register_process(&process, Some(&target.display_name));
        assert_eq!(result2, None);
    }

    // TODO: add tests for multiple job matches
    // #[rstest]
    // fn test_multiple_job_matches(
    //     mut pipeline_manager: TargetPipelineManager,
    //     test_targets: &Vec<Target>,
    // ) {
    // }
}
