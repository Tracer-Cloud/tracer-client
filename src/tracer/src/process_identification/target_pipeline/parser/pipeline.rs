#[derive(Debug, Clone)]
pub struct Pipeline {
    pub id: String,
    pub description: Option<String>,
    pub repo: Option<String>,
    pub language: Option<String>,
    pub version: Option<Version>,
    pub steps: Option<Vec<Step>>,
    pub optional_steps: Option<Vec<Step>>,
    pub dependencies: Option<Dependencies>,
}

#[derive(Debug, Clone)]
pub struct Version {
    pub min: Option<String>,
    pub max: Option<String>,
    pub exact: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Subworkflow {
    pub id: String,
    pub description: Option<String>,
    pub steps: Option<Vec<Step>>,
    pub optional_steps: Option<Vec<Step>>,
}

#[derive(Debug, Clone)]
pub struct Job {
    pub id: String,
    pub description: Option<String>,
    pub rules: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum Step {
    Job(String),
    OptionalJob(String),
    Subworkflow(String),
    OptionalSubworkflow(String),
    And(Vec<Step>),
    Or(Vec<Step>),
}

#[derive(Debug, Clone)]
pub struct Dependencies {
    pub subworkflows: Option<Vec<Subworkflow>>,
    pub jobs: Option<Vec<Job>>,
    pub parent: Option<Box<&'static Dependencies>>,
}

impl Dependencies {
    pub fn new(
        subworkflows: Option<Vec<Subworkflow>>,
        jobs: Option<Vec<Job>>,
        parent: Option<&'static Dependencies>,
    ) -> Self {
        Self {
            subworkflows,
            jobs,
            parent: parent.map(Box::new),
        }
    }
}
