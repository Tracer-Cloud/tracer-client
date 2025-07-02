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
pub struct Task {
    pub id: String,
    pub description: Option<String>,
    pub rules: Vec<String>,
    pub optional_rules: Option<Vec<String>>,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum Step {
    Task(String),
    OptionalTask(String),
    Subworkflow(String),
    OptionalSubworkflow(String),
    And(Vec<Step>),
    Or(Vec<Step>),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Dependencies {
    pub subworkflows: HashMap<String, Subworkflow>,
    pub tasks: HashMap<String, Task>,
    pub parent: Option<&'static Dependencies>,
}

impl Dependencies {
    pub fn new(
        subworkflows: Option<Vec<Subworkflow>>,
        tasks: Option<Vec<Task>>,
        parent: Option<&'static Dependencies>,
    ) -> Self {
        Self {
            subworkflows: subworkflows
                .map(|v| v.into_iter().map(|s| (s.id.clone(), s)).collect())
                .unwrap_or_default(),
            tasks: tasks
                .map(|v| v.into_iter().map(|s| (s.id.clone(), s)).collect())
                .unwrap_or_default(),
            parent,
        }
    }

    pub fn get_task(&self, id: &str) -> Option<&Task> {
        self.tasks
            .get(id)
            .or_else(|| self.parent.as_ref().and_then(|p| p.get_task(id)))
    }

    pub fn get_subworkflow(&self, id: &str) -> Option<&Subworkflow> {
        self.subworkflows
            .get(id)
            .or_else(|| self.parent.as_ref().and_then(|p| p.get_subworkflow(id)))
    }
}
