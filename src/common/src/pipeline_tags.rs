use clap::Args;

#[derive(Args, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PipelineTags {
    /// Enviroment: Execution Context. E.g: ci-cd, sandbox, local
    #[clap(long, short)]
    pub environment: String,

    /// pipeline type: Used to identify the type of the pipeline.
    #[clap(long)]
    pub pipeline_type: String,

    /// User Operator: Responsible individual executing the pipeline
    #[clap(long)]
    pub user_operator: String,

    /// Department: Organizational unit, e.g., "Research"
    #[clap(long, default_value = "Research")]
    pub department: String,

    /// Team: Business division, e.g., "Oncology"
    #[clap(long, default_value = "Oncology")]
    pub team: String,

    /// Others: Any other tag you'd like to specify
    #[clap(long, value_delimiter = ',')]
    pub others: Vec<String>,
}

impl Default for PipelineTags {
    fn default() -> Self {
        Self {
            environment: "local".into(),
            pipeline_type: "generic".into(),
            user_operator: "tracer_user".into(),
            department: "dev".into(),
            team: "dev".into(),
            others: vec![],
        }
    }
}
