use crate::process_identification::target_pipeline::parser::pipeline::{
    Dependencies, Job, Pipeline, Step,
};
use crate::process_identification::target_pipeline::parser::yaml_rules_parser::load_yaml_pipelines;
use crate::process_identification::target_process::target::Target;
use multi_index_map::MultiIndexMap;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use tracer_ebpf::ebpf_trigger::ProcessStartTrigger;
use tracing::trace;

pub const JOB_SCORE_THRESHOLD: f64 = 0.9;

/// A job that is matched to a set of processes that have been started.
#[derive(Debug, Clone, PartialEq)]
pub struct JobMatch {
    /// The ID of the job that was matched.
    pub id: String,
    /// The description of the job that was matched.
    pub description: Option<String>,
    /// The PIDs that have been matched to this job.
    pub pids: Vec<usize>,
    /// The score of the job that was matched.
    pub score: f64,
}

pub struct TargetPipelineManager {
    pipelines: Vec<Pipeline>,
    jobs: Jobs,
    job_pids: MultiIndexJobPidMap,
    pid_to_process: HashMap<usize, ProcessRule>,
}

impl TargetPipelineManager {
    fn new<P: AsRef<Path>>(
        embedded_yaml: Option<&str>,
        fallback_paths: &[P],
        targets: &Vec<Target>,
    ) -> Self {
        let pipelines = load_yaml_pipelines(embedded_yaml, fallback_paths);
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
            pipelines,
            jobs,
            job_pids: MultiIndexJobPidMap::default(),
            pid_to_process: HashMap::new(),
        }
    }

    pub fn register_process(
        &mut self,
        process: &ProcessStartTrigger,
        matched_target: Option<&Target>,
    ) -> Option<JobMatch> {
        // TODO: this can lead to false-negatives if PIDs are reused.
        if self.pid_to_process.contains_key(&process.pid) {
            trace!("PID {} is already registered", process.pid);
            return None;
        }

        let (rule, matched) = if let Some(matched) = matched_target {
            (&matched.display_name, true)
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
                    let score = job.rules.len() as f64 / job_pids.len() as f64;
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

/// A bi-directional many-to-many mapping between jobs and PIDs.
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