use crate::utils::env::{self, USER_ID_ENV_VAR};
use clap::Args;

pub const PIPELINE_TYPE_ENV_VAR: &str = "TRACER_PIPELINE_TYPE";
pub const ENVIRONMENT_ENV_VAR: &str = "TRACER_ENVIRONMENT";
pub const DEPARTMENT_ENV_VAR: &str = "TRACER_DEPARTMENT";
pub const TEAM_ENV_VAR: &str = "TRACER_TEAM";
pub const ORGANIZATION_ID_ENV_VAR: &str = "TRACER_ORGANIZATION_ID";

#[derive(Args, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PipelineTags {
    /// pipeline execution context (e.g., ci-cd, sandbox, local)
    #[clap(long, short, value_parser = StringValueParser, env = ENVIRONMENT_ENV_VAR)]
    pub environment: Option<String>,

    /// pipeline type (e.g., preprocessing, RNA-seq, single-cell)
    #[clap(long, value_parser = StringValueParser, env = PIPELINE_TYPE_ENV_VAR)]
    pub pipeline_type: Option<String>,

    /// organizational unit (e.g., "Research")
    #[clap(long, value_parser = StringValueParser, env = DEPARTMENT_ENV_VAR, default_value = "Research")]
    pub department: String,

    /// business division (e.g., "Oncology")
    #[clap(long, value_parser = StringValueParser, env = TEAM_ENV_VAR, default_value = "Oncology")]
    pub team: String,

    /// organization ID
    #[clap(long, value_parser = StringValueParser, env = ORGANIZATION_ID_ENV_VAR)]
    pub organization_id: Option<String>,

    /// user ID to associate this session with your account
    #[clap(long, env = USER_ID_ENV_VAR)]
    pub user_id: Option<String>,

    /// other tags you'd like to attach to this session
    #[clap(long, value_parser = StringValueParser, value_delimiter = ',')]
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
            others: vec![],
        }
    }
}
