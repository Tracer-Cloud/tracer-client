use clap::Args;

#[derive(Default, Args, Debug, Clone)]
pub struct TracerCliInitArgs {
    /// pipeline name to init the daemon with
    #[clap(long, short)]
    pub pipeline_name: String,

    /// Run Identifier: this is used group same pipeline runs on different computers.
    /// Context: aws batch can run same pipeline on multiple machines for speed
    #[clap(long)]
    pub run_id: Option<String>,

    #[clap(flatten)]
    pub tags: PipelineTags,
}

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
