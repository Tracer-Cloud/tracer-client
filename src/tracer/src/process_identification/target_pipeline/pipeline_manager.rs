use crate::process_identification::target_pipeline::parser::pipeline::{
    Dependencies, Pipeline, Step, Task,
};
use crate::process_identification::target_pipeline::parser::yaml_rules_parser::load_pipelines_from_yamls;
use crate::process_identification::target_process::target::Target;
use crate::process_identification::target_process::target_match::MatchType;
use crate::utils::yaml::YamlFile;
use anyhow::Result;
use multi_index_map::MultiIndexMap;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use tracer_ebpf::ebpf_trigger::ProcessStartTrigger;
use tracing::trace;

pub const TASK_SCORE_THRESHOLD: f64 = 0.9;

/// A task that is matched to a set of processes that have been started.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskMatch {
    /// The ID of the task that was matched - this is used for labeling the process group in the UI.
    pub id: String,
    /// The description of the task that was matched.
    pub description: Option<String>,
    /// The PIDs that have been matched to this task.
    pub pids: Vec<usize>,
    /// The score of the task that was matched.
    pub score: f64,
    /// Total number of rules in the matched task.
    pub total_rules: usize,
}

impl std::fmt::Display for TaskMatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "TaskMatch(id: {}, description: {:?}, pids: {:?}, score: {})",
            self.id, self.description, self.pids, self.score
        )
    }
}

pub struct TargetPipelineManager {
    _pipelines: Vec<Pipeline>,
    tasks: Tasks,
    task_pids: MultiIndexTaskPidMap,
    matched_task_pids: MultiIndexTaskPidMap,
    pid_to_process: HashMap<usize, ProcessRule>,
}

impl TargetPipelineManager {
    pub fn new(rule_files: &[YamlFile], _targets: &[Target]) -> Result<Self> {
        let pipelines = load_pipelines_from_yamls(rule_files);
        let mut tasks = Tasks::default();
        for pipeline in pipelines.iter() {
            let dependencies = &pipeline.dependencies;
            if let Some(steps) = &pipeline.steps {
                tasks.add_steps(steps, dependencies, false)?;
            }
            if let Some(steps) = &pipeline.optional_steps {
                tasks.add_steps(steps, dependencies, true)?;
            }
        }
        Ok(Self {
            _pipelines: pipelines,
            tasks,
            task_pids: MultiIndexTaskPidMap::default(),
            matched_task_pids: MultiIndexTaskPidMap::default(),
            pid_to_process: HashMap::new(),
        })
    }

    /// Registers a process with the pipeline manager. This associates the process with all tasks
    /// that include the matched target (if any, falling back to the process's command name). If
    /// any task's score rises above the match threshold after adding the process, then the best
    /// matching task is returned. If the best match has a perfect score, then all the pids are
    /// dissociated from any other tasks.
    pub fn register_process(
        &mut self,
        process: &ProcessStartTrigger,
        matched_target: Option<&String>,
    ) -> Option<TaskMatch> {
        if self.pid_to_process.contains_key(&process.pid) {
            // TODO: this can lead to false-negatives if PIDs are reused.
            trace!("PID {} is already registered", process.pid);
            return None;
        };
        let (rule, matched) = if let Some(display_name) = matched_target {
            (display_name, true)
        } else {
            (&process.comm, false)
        };
        if let Some(tasks) = self.tasks.get_tasks_with(rule) {
            // add the PID to the set we're tracking
            self.pid_to_process.insert(
                process.pid,
                ProcessRule {
                    name: rule.clone(),
                    matched,
                },
            );
            // find any tasks that exceed the score threshold after adding the rule
            let mut matched_tasks = tasks
                .iter()
                .filter_map(|(task, match_type)| {
                    // TODO: should we check that the rule associated with the PID is not one
                    // that has already been recognized? Sometimes, the same command will be run
                    // multiple times in the same task. We could require a separate rule entry in
                    // the task definition for each time the command is run, or have a way to
                    // specify the cardinality of the rule within the task.

                    // if the rule is specialized, check if the additional conditions match the
                    // process
                    if let Some(match_type) = match_type {
                        if !match_type.matches(process) {
                            return None;
                        }
                    }
                    // if the task already has a PID with the same rule, don't add it again
                    for existing_task_pid in self.task_pids.get_by_task_id(&task.id) {
                        if self
                            .pid_to_process
                            .get(&existing_task_pid.pid)
                            .is_some_and(|p| p.name == *rule && p.matched == matched)
                        {
                            return None;
                        }
                    }
                    // add the PID to the list for candidate task
                    self.task_pids.insert(TaskPid {
                        task_id: task.id.clone(),
                        pid: process.pid,
                    });
                    let task_pids = self.task_pids.get_by_task_id(&task.id);
                    // For now score is just the fraction of rules that have been observed.
                    // TODO: weight score based on whether the rule is optional or not.
                    let (score, total_rules): (f64, usize) = {
                        let num_matched = task_pids.len() as f64;
                        let total = task.rules.len()
                            + task.optional_rules.as_ref().map(|v| v.len()).unwrap_or(0);
                        ((num_matched / total as f64), total)
                    };
                    if score > TASK_SCORE_THRESHOLD {
                        let pids: Vec<usize> =
                            task_pids.iter().map(|task_pid| task_pid.pid).collect();
                        Some((task, pids, score, total_rules))
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();
            if matched_tasks.is_empty() {
                return None;
            }
            // if there are multiple matches, pick the one with the highest score
            if matched_tasks.len() > 1 {
                matched_tasks.sort_by(|a, b| match a.2.partial_cmp(&b.2) {
                    Some(Ordering::Equal) => a.3.cmp(&b.3), // use number of tasks as tiebreaker
                    Some(o) => o,
                    None => panic!("Score comparison failed"),
                });
            }
            let (best_match, pids, score, total_rules) = matched_tasks.pop().unwrap();
            let id = best_match.id.clone();
            if score >= 1.0 {
                // If the match is perfect (i.e. all rules have been matched to processes) then:
                // 1) find and remove all tasks associated with the matched PIDs
                let tasks_to_remove: HashSet<String> = pids
                    .iter()
                    .flat_map(|pid| {
                        self.matched_task_pids
                            .get_by_pid(pid)
                            .iter()
                            .map(|task_pid| task_pid.task_id.clone())
                            .collect::<Vec<_>>()
                    })
                    .collect();
                for task in tasks_to_remove {
                    self.matched_task_pids.remove_by_task_id(&task);
                }
                // 2) Add the best match to the matched tasks
                for pid in pids.iter() {
                    self.matched_task_pids.insert(TaskPid {
                        task_id: id.clone(),
                        pid: *pid,
                    });
                }
            }
            return Some(TaskMatch {
                id: id.clone(),
                description: best_match.description.clone(),
                pids,
                score,
                total_rules,
            });
        }

        None
    }

    pub fn matched_tasks(&self) -> HashSet<&String> {
        self.matched_task_pids
            .iter()
            .map(|(_, task_pid)| &task_pid.task_id)
            .collect()
    }
}

impl Default for TargetPipelineManager {
    fn default() -> Self {
        const RULE_FILES: &[YamlFile] = &[YamlFile::Embedded(include_str!(
            "yml_rules/tracer.pipelines.yml"
        ))];
        Self::new(RULE_FILES, &Vec::new()).expect("Failed to create default pipeline manager")
    }
}

/// A bidirectional many-to-many mapping between jobs and PIDs.
#[derive(MultiIndexMap, Debug)]
struct TaskPid {
    #[multi_index(hashed_non_unique)]
    task_id: String,
    #[multi_index(hashed_non_unique)]
    pid: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ProcessRule {
    name: String,
    matched: bool,
}

#[derive(Debug, Clone, Default)]
struct Tasks {
    tasks: HashMap<String, Task>,
    rule_to_task: HashMap<String, HashSet<(String, Option<MatchType>)>>,
}

impl Tasks {
    fn add_steps(
        &mut self,
        steps: &Vec<Step>,
        dependencies: &Dependencies,
        optional: bool,
    ) -> Result<()> {
        for step in steps {
            match step {
                Step::Task(id) => self.add_task(id, dependencies, optional)?,
                Step::OptionalTask(id) => self.add_task(id, dependencies, true)?,
                Step::Subworkflow(id) => self.add_subworkflow(id, dependencies, optional)?,
                Step::OptionalSubworkflow(id) => self.add_subworkflow(id, dependencies, true)?,
                Step::And(steps) => self.add_steps(steps, dependencies, optional)?,
                Step::Or(steps) => self.add_steps(steps, dependencies, optional)?,
            };
        }
        Ok(())
    }

    fn add_task(
        &mut self,
        id: &String,
        dependencies: &Dependencies,
        _optional: bool,
    ) -> Result<()> {
        if let Some(task) = dependencies.get_task(id) {
            if !self.tasks.contains_key(id) {
                self.tasks.insert(id.clone(), task.clone());
            }
            for rule in &task.rules {
                if let Some(tasks) = self.rule_to_task.get_mut(rule) {
                    tasks.insert((task.id.clone(), None));
                } else {
                    self.rule_to_task
                        .insert(rule.clone(), HashSet::from([(task.id.clone(), None)]));
                }
            }
            if let Some(optional_rules) = task.optional_rules.as_ref() {
                for rule in optional_rules {
                    if let Some(tasks) = self.rule_to_task.get_mut(rule) {
                        tasks.insert((task.id.clone(), None));
                    } else {
                        self.rule_to_task
                            .insert(rule.clone(), HashSet::from([(task.id.clone(), None)]));
                    }
                }
            }
            if let Some(specialized_rules) = task.specialized_rules.as_ref() {
                for rule in specialized_rules {
                    if let Some(tasks) = self.rule_to_task.get_mut(&rule.name) {
                        tasks.insert((task.id.clone(), Some(rule.condition.clone().try_into()?)));
                    } else {
                        self.rule_to_task.insert(
                            rule.name.clone(),
                            HashSet::from([(
                                task.id.clone(),
                                Some(rule.condition.clone().try_into()?),
                            )]),
                        );
                    }
                }
            }
            if let Some(optional_specialized_rules) = task.optional_specialized_rules.as_ref() {
                for rule in optional_specialized_rules {
                    if let Some(tasks) = self.rule_to_task.get_mut(&rule.name) {
                        tasks.insert((task.id.clone(), Some(rule.condition.clone().try_into()?)));
                    } else {
                        self.rule_to_task.insert(
                            rule.name.clone(),
                            HashSet::from([(
                                task.id.clone(),
                                Some(rule.condition.clone().try_into()?),
                            )]),
                        );
                    }
                }
            }
        }
        Ok(())
    }

    fn add_subworkflow(
        &mut self,
        id: &str,
        dependencies: &Dependencies,
        optional: bool,
    ) -> Result<()> {
        if let Some(subworkflow) = dependencies.get_subworkflow(id) {
            if let Some(steps) = &subworkflow.steps {
                self.add_steps(steps, dependencies, optional)?;
            }
            if let Some(steps) = &subworkflow.optional_steps {
                self.add_steps(steps, dependencies, true)?;
            }
        }
        Ok(())
    }

    fn get_tasks_with(&self, rule: &str) -> Option<HashSet<(&Task, &Option<MatchType>)>> {
        self.rule_to_task.get(rule).map(|tasks| {
            tasks
                .iter()
                .map(|task| (self.tasks.get(&task.0).unwrap(), &task.1))
                .collect()
        })
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

    /// Fixture that creates test targets for the pipeline rules
    #[fixture]
    #[once]
    fn test_targets() -> Vec<Target> {
        vec![
            Target::with_display_name(
                MatchType::And(Vec::from([
                    MatchType::ProcessNameIs("gzip".to_string()),
                    MatchType::FirstArgIs("-cd".to_string()),
                    MatchType::CommandContains(".gtf.gz".to_string()),
                ])),
                "gunzip_gtf".to_string(),
            ),
            Target::with_display_name(
                MatchType::And(Vec::from([
                    MatchType::ProcessNameIs("gzip".to_string()),
                    MatchType::FirstArgIs("-cd".to_string()),
                    MatchType::CommandContains(".gff.gz".to_string()),
                ])),
                "gunzip_gff".to_string(),
            ),
            Target::with_display_name(
                MatchType::ProcessNameIs("gffread".to_string()),
                "gffread".to_string(),
            ),
            Target::with_display_name(
                MatchType::ProcessNameIs("bbsplit.sh".to_string()),
                "bbsplit".to_string(),
            ),
            Target::with_display_name(
                MatchType::ProcessNameIs("jshell".to_string()),
                "jshell".to_string(),
            ),
            Target::with_display_name(
                MatchType::And(Vec::from([
                    MatchType::ProcessNameIs("samtools".to_string()),
                    MatchType::FirstArgIs("faidx".to_string()),
                ])),
                "samtools faidx".to_string(),
            ),
            Target::with_display_name(
                MatchType::And(Vec::from([
                    MatchType::ProcessNameIs("STAR".to_string()),
                    MatchType::CommandContains("--runMode genomeGenerate".to_string()),
                ])),
                "STAR index".to_string(),
            ),
        ]
    }

    /// Fixture that creates a TargetPipelineManager with default pipeline rules
    #[fixture]
    fn pipeline_manager(test_targets: &Vec<Target>) -> TargetPipelineManager {
        TargetPipelineManager::new(PIPELINE_YAML_PATH, test_targets)
            .expect("Error creating pipeline manager")
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
            .find(|target| target.display_name() == display_name)
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
        let result =
            pipeline_manager.register_process(&process, Some(&target.display_name().to_string()));

        assert_eq!(
            result,
            Some(TaskMatch {
                id: "GUNZIP_GTF".to_string(),
                description: Some("Unzip the GTF file.".to_string()),
                pids: vec![1001],
                score: 1.0,
                total_rules: 1,
            })
        );
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
        let result1 = manager.register_process(
            &jshell_process,
            Some(&jshell_target.display_name().to_string()),
        );

        // Should return None since we need more processes
        assert_eq!(result1, None);

        // Register bbsplit process
        let bbsplit_process = create_process_trigger("bbsplit.sh", 1001);
        let bbsplit_target = find_target_by_display_name(&test_targets, "bbsplit").unwrap();
        let result2 = manager.register_process(
            &bbsplit_process,
            Some(&bbsplit_target.display_name().to_string()),
        );

        // Should return a match for the BBMAP_BBSPLIT task since it has score 1.0 (both rules matched)
        assert!(result2.is_some());
        let task_match = result2.unwrap();
        assert_eq!(task_match.id, "BBMAP_BBSPLIT");
        assert_eq!(
            task_match.description,
            Some("Split the FASTQ file into smaller chunks.".to_string())
        );
        assert_eq!(task_match.score, 1.0);
        assert_eq!(task_match.pids, vec![1002, 1001]);
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
        let result1 = manager.register_process(
            &samtools_process,
            Some(&samtools_target.display_name().to_string()),
        );

        let expected_match = Some(TaskMatch {
            id: "SAMTOOLS_FAIDX".to_string(),
            description: Some("Index a FASTA file".to_string()),
            pids: vec![1002],
            score: 1.0,
            total_rules: 1,
        });

        // Should return the samtools faidx task since it has score 1.0 (single rule matched)
        assert_eq!(result1, expected_match);

        // Register STAR process
        let star_process = create_process_trigger("STAR --runMode genomeGenerate", 1001);
        let star_target = find_target_by_display_name(&test_targets, "STAR index").unwrap();
        let result2 =
            manager.register_process(&star_process, Some(&star_target.display_name().to_string()));

        // Should return a match for the STAR_GENOMEGENERATE task since it has score 1.0 (both
        //rules matched) and it has more rules than the samtools faidx task.
        assert!(result2.is_some());
        let task_match = result2.unwrap();
        assert_eq!(task_match.id, "STAR_GENOMEGENERATE");
        assert_eq!(
            task_match.description,
            Some("Generate the genome index for STAR.".to_string())
        );
        assert_eq!(task_match.score, 1.0);
        assert_eq!(task_match.pids, vec![1002, 1001]);

        let matched_tasks = manager.matched_tasks();
        assert_eq!(matched_tasks.len(), 1);
        assert!(matched_tasks.contains(&"STAR_GENOMEGENERATE".to_string()));
    }

    #[rstest]
    fn test_duplicate_pid_registration(
        mut pipeline_manager: TargetPipelineManager,
        test_targets: &Vec<Target>,
    ) {
        // Register a gunzip process that matches the gunzip_gtf rule
        let process = create_process_trigger("gzip -cd foo.gtf.gz", 1001);
        let target = find_target_by_display_name(&test_targets, "gunzip_gtf").unwrap();
        let result1 =
            pipeline_manager.register_process(&process, Some(&target.display_name().to_string()));

        let expected_match = Some(TaskMatch {
            id: "GUNZIP_GTF".to_string(),
            description: Some("Unzip the GTF file.".to_string()),
            pids: vec![1001],
            score: 1.0,
            total_rules: 1,
        });
        assert_eq!(result1, expected_match);

        // Register the same process again - should be ignored
        let result2 =
            pipeline_manager.register_process(&process, Some(&target.display_name().to_string()));
        assert_eq!(result2, None);
    }

    // TODO: add tests for multiple task matches
    // #[rstest]
    // fn test_multiple_task_matches(
    //     mut pipeline_manager: TargetPipelineManager,
    //     test_targets: &Vec<Target>,
    // ) {
    // }
}
