use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Pipeline {
    pub id: String,
    pub description: Option<String>,
    pub repo: Option<String>,
    pub language: Option<String>,
    pub version: Option<Version>,
    pub steps: Option<Vec<Step>>,
    pub optional_steps: Option<Vec<Step>>,
    pub dependencies: Dependencies,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct Version {
    pub min: Option<String>,
    pub max: Option<String>,
    pub exact: Option<String>,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct Subworkflow {
    pub id: String,
    pub description: Option<String>,
    pub steps: Option<Vec<Step>>,
    pub optional_steps: Option<Vec<Step>>,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct Job {
    pub id: String,
    pub description: Option<String>,
    pub rules: Vec<String>,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum Step {
    Job(String),
    OptionalJob(String),
    Subworkflow(String),
    OptionalSubworkflow(String),
    And(Vec<Step>),
    Or(Vec<Step>),
}

#[derive(Debug, Clone, Default)]
pub struct Dependencies {
    pub subworkflows: HashMap<String, Subworkflow>,
    pub jobs: HashMap<String, Job>,
    pub parent: Option<Box<&'static Dependencies>>,
}

impl Dependencies {
    pub fn new(
        subworkflows: Option<Vec<Subworkflow>>,
        jobs: Option<Vec<Job>>,
        parent: Option<&'static Dependencies>,
    ) -> Self {
        Self {
            subworkflows: subworkflows
                .map(|v| v.into_iter().map(|s| (s.id.clone(), s)).collect())
                .unwrap_or_default(),
            jobs: jobs
                .map(|v| v.into_iter().map(|s| (s.id.clone(), s)).collect())
                .unwrap_or_default(),
            parent: parent.map(Box::new),
        }
    }

    pub fn get_job(&self, id: &str) -> Option<&Job> {
        self.jobs.get(id)
    }

    pub fn get_subworkflow(&self, id: &str) -> Option<&Subworkflow> {
        self.subworkflows.get(id)
    }
}