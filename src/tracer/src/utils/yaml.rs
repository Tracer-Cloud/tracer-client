use crate::utils::file_system::TrustedFile;
use anyhow::{anyhow, bail, Result};
use std::fs;
use std::path::Path;
use yaml_rust2::YamlLoader;

// re-export Yaml for convenience
pub use yaml_rust2::Yaml;

#[derive(Clone, Debug)]
pub struct YamlFile(TrustedFile);

impl YamlFile {
    pub const fn from_embedded_str(contents: &'static str) -> Self {
        Self(TrustedFile::from_embedded_str(contents))
    }

    pub const fn from_src_path(path: &'static str) -> Self {
        Self(TrustedFile::from_src_path(path))
    }

    pub fn load<T: TryFrom<Yaml, Error = anyhow::Error>>(&self, key: &str) -> Result<Vec<T>> {
        let yaml_str = self.0.read_to_string()?;
        load_from_yaml_array_str(&yaml_str, key)
    }
}

pub fn load_from_yaml_array_file<P: AsRef<Path>, T: TryFrom<Yaml, Error = anyhow::Error>>(
    path: P,
    key: &str,
) -> Result<Vec<T>> {
    let yaml_str = fs::read_to_string(path.as_ref())?;
    load_from_yaml_array_str(&yaml_str, key)
}

pub fn load_from_yaml_array_str<T: TryFrom<Yaml, Error = anyhow::Error>>(
    yaml_str: &str,
    key: &str,
) -> Result<Vec<T>> {
    let docs = YamlLoader::load_from_str(yaml_str)?;
    docs.into_iter()
        .next()
        .ok_or(anyhow!("Empty yaml file"))?
        .into_hash()
        .ok_or(anyhow!("Expected top-level element to be a hash"))?
        .remove(&Yaml::String(key.into()))
        .ok_or(anyhow!("Missing top-level key {}", key))?
        .into_iter()
        .map(|yaml| yaml.try_into())
        .collect()
}

pub trait YamlExt: Sized {
    fn required(&self, key: &'static str) -> Result<&Yaml>;

    fn optional(&self, key: &'static str) -> Option<&Yaml>;

    fn required_string(&self, key: &'static str) -> Result<String>;

    fn optional_string(&self, key: &'static str) -> Result<Option<String>>;

    fn required_vec(&self, key: &'static str) -> Result<&Vec<Self>>;

    fn optional_vec(&self, key: &'static str) -> Result<Option<&Vec<Self>>>;

    fn to_string(&self) -> Result<String>;

    fn to_usize(&self) -> Result<usize>;
}

impl YamlExt for Yaml {
    fn required(&self, key: &'static str) -> Result<&Yaml> {
        let value = &self[key];
        if value.is_badvalue() {
            bail!("Missing key {}", key)
        } else {
            Ok(value)
        }
    }

    fn optional(&self, key: &'static str) -> Option<&Yaml> {
        let value = &self[key];
        if value.is_badvalue() {
            None
        } else {
            Some(value)
        }
    }

    fn required_string(&self, key: &'static str) -> Result<String> {
        match &self[key] {
            Yaml::String(s) => Ok(s.clone()),
            Yaml::BadValue => bail!("Missing key {}", key),
            _ => bail!("Expected {} to be a string", key),
        }
    }

    fn optional_string(&self, key: &'static str) -> Result<Option<String>> {
        match &self[key] {
            Yaml::String(s) => Ok(Some(s.clone())),
            Yaml::BadValue => Ok(None),
            _ => bail!("Expected {} to be a string", key),
        }
    }

    fn required_vec(&self, key: &'static str) -> Result<&Vec<Self>> {
        match &self[key] {
            Yaml::Array(v) => Ok(v),
            Yaml::BadValue => bail!("Missing key {}", key),
            _ => bail!("Expected {} to be an array", key),
        }
    }

    fn optional_vec(&self, key: &'static str) -> Result<Option<&Vec<Self>>> {
        match &self[key] {
            Yaml::Array(v) => Ok(Some(v)),
            Yaml::BadValue => Ok(None),
            _ => bail!("Expected {} to be an array", key),
        }
    }

    fn to_string(&self) -> Result<String> {
        match self {
            Yaml::String(s) => Ok(s.clone()),
            _ => bail!("Expected a string"),
        }
    }

    fn to_usize(&self) -> Result<usize> {
        match self {
            Yaml::Integer(i) => Ok(*i as usize),
            _ => bail!("Expected a number"),
        }
    }
}
