use crate::utils::env;
use clap::Args;

pub const PIPELINE_TYPE_ENV_VAR: &str = "TRACER_PIPELINE_TYPE";
pub const ENVIRONMENT_ENV_VAR: &str = "TRACER_ENVIRONMENT";
pub const USER_ID_ENV_VAR: &str = "TRACER_USER_ID";
pub const DEPARTMENT_ENV_VAR: &str = "TRACER_DEPARTMENT";
pub const TEAM_ENV_VAR: &str = "TRACER_TEAM";
pub const ORGANIZATION_ID_ENV_VAR: &str = "TRACER_ORGANIZATION_ID";

#[derive(Args, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PipelineTags {
    /// Environment: Execution Context. E.g: ci-cd, sandbox, local
    #[clap(long, short, env = ENVIRONMENT_ENV_VAR)]
    pub environment: Option<String>,

    /// pipeline type: Used to identify the type of the pipeline.
    #[clap(long, env = PIPELINE_TYPE_ENV_VAR)]
    pub pipeline_type: Option<String>,

    /// User ID: Responsible individual executing the pipeline, is the Clerk user id in the webapp
    #[clap(long, env = USER_ID_ENV_VAR)]
    pub user_id: Option<String>,

    /// Department: Organizational unit, e.g., "Research"
    #[clap(long, env = DEPARTMENT_ENV_VAR, default_value = "Research")]
    pub department: String,

    /// Team: Business division, e.g., "Oncology"
    #[clap(long, env = TEAM_ENV_VAR, default_value = "Oncology")]
    pub team: String,

    /// Organization ID
    #[clap(long, env = ORGANIZATION_ID_ENV_VAR)]
    pub organization_id: Option<String>,

    /// Instance Type: EC2 instance type (e.g., t2.micro, m5.large)
    #[clap(long)]
    pub instance_type: Option<String>,

    /// Environment Type: Detected environment type (e.g., GitHub Actions, AWS EC2, Local)
    #[clap(long)]
    pub environment_type: Option<String>,

    /// Others: Any other tag you'd like to specify
    #[clap(long, value_delimiter = ',')]
    pub others: Vec<String>,
}

impl Default for PipelineTags {
    fn default() -> Self {
        Self {
            environment: Some("local".into()),
            pipeline_type: Some("generic".into()),
            user_id: env::get_env_var(USER_ID_ENV_VAR),
            department: "dev".into(),
            team: "dev".into(),
            organization_id: None,
            instance_type: None,
            environment_type: None,
            others: vec![],
        }
    }
}
