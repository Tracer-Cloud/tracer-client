use clap::Args;

#[derive(Args, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PipelineTags {
    /// Enviroment: Execution Context. E.g: ci-cd, sandbox, local
    #[clap(long, short)]
    pub environment: Option<String>,

    /// pipeline type: Used to identify the type of the pipeline.
    #[clap(long)]
    pub pipeline_type: Option<String>,

    /// User Operator: Responsible individual executing the pipeline
    #[clap(long)]
    pub user_operator: Option<String>,

    /// Department: Organizational unit, e.g., "Research"
    #[clap(long, default_value = "Research")]
    pub department: String,

    /// Team: Business division, e.g., "Oncology"
    #[clap(long, default_value = "Oncology")]
    pub team: String,

    /// Organization ID
    #[clap(long)]
    pub organization_id: Option<String>,

    /// Others: Any other tag you'd like to specify
    #[clap(long, value_delimiter = ',')]
    pub others: Vec<String>,
}

impl Default for PipelineTags {
    fn default() -> Self {
        Self {
            environment: Some("local".into()),
            pipeline_type: Some("generic".into()),
            user_operator: Some(whoami::username()),
            department: "dev".into(),
            team: "dev".into(),
            organization_id: None,
            others: vec![],
        }
    }
}
