use std::fmt;
use std::str::FromStr;

#[derive(Clone, Debug)]
pub enum TracerVersion {
    Development,
    Production,
    Feature(String),
}

impl FromStr for TracerVersion {
    type Err = anyhow::Error;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        match input.to_lowercase().as_str() {
            "development" | "dev" => Ok(Self::Development),
            "production" | "prod" => Ok(Self::Production),
            _ => Ok(Self::Feature(input.to_string())),
        }
    }
}

impl fmt::Display for TracerVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TracerVersion::Development => write!(f, "development"),
            TracerVersion::Production => write!(f, "production"),
            TracerVersion::Feature(name) => write!(f, "Custom Branch({name})"),
        }
    }
}
