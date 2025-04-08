use config::{Value, ValueKind};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AwsConfig {
    Profile(String),
    RoleArn(String),
    Env,
}

impl From<AwsConfig> for ValueKind {
    fn from(value: AwsConfig) -> Self {
        match value {
            AwsConfig::Profile(profile) => {
                let mut table = HashMap::new();
                table.insert(
                    "profile".to_string(),
                    Value::new(None, Self::String(profile.to_owned())),
                );
                Self::Table(table)
            }
            AwsConfig::RoleArn(role) => {
                let mut table = HashMap::new();
                table.insert(
                    "role_arn".to_string(),
                    Value::new(None, Self::String(role.to_owned())),
                );
                Self::Table(table)
            }
            AwsConfig::Env => Self::String("env".to_string()),
        }
    }
}
