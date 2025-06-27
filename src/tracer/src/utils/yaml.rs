use anyhow::{anyhow, bail, Result};
use std::fs;
use std::path::Path;
use yaml_rust2::YamlLoader;
// re-export Yaml for convenience
use tracing::trace;
pub use yaml_rust2::Yaml;

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

pub struct YamlVecLoader<'a, P: AsRef<Path>> {
    pub module: &'a str,
    pub key: &'a str,
    pub fallback_paths: &'a [P],
    pub embedded_yaml: Option<&'a str>,
}

impl<P: AsRef<Path>> YamlVecLoader<'_, P> {
    pub fn load<T: TryFrom<Yaml, Error = anyhow::Error>>(&self) -> Vec<T> {
        if let Some(embedded_str) = self.embedded_yaml {
            match load_from_yaml_array_str(embedded_str, self.key) {
                Ok(loaded) if !loaded.is_empty() => return loaded,
                Ok(_) => {
                    trace!("Embedded YAML is empty");
                }
                Err(e) => {
                    trace!("[{}] Failed to load embedded YAML: {}", self.module, e);
                }
            }
        }
        for path in self.fallback_paths.iter() {
            match load_from_yaml_array_file(path, self.key) {
                Ok(loaded) if !loaded.is_empty() => return loaded,
                Ok(_) => {
                    trace!("YAML file is empty: {}", path.as_ref().display());
                }
                Err(e) => {
                    trace!(
                        "[{}] Failed to load YAML from {:?}: {}",
                        self.module,
                        path.as_ref(),
                        e
                    );
                }
            }
        }
        Vec::new()
    }
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
