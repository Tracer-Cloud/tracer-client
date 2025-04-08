use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum AwsConfig {
    Profile(String),
    RoleArn(String),
    Env,
}
