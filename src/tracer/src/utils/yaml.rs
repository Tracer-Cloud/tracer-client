use anyhow::{anyhow, bail, Result};
use std::fs;
use std::path::Path;
use yaml_rust2::YamlLoader;
// re-export Yaml for convenience
use std::collections::HashSet;
use std::hash::Hash;
use tracing::error;
pub use yaml_rust2::Yaml;

pub fn load_from_yaml_array_files<T: TryFrom<Yaml, Error = anyhow::Error> + Hash + Eq>(
    yaml_files: &[YamlFile],
    key: &str,
) -> HashSet<T> {
    let mut result = HashSet::new();
    for yaml_file in yaml_files {
        if let Err(e) = yaml_file.load_into(key, &mut result) {
            error!("Error loading yaml file {:?}: {}", yaml_file, e);
        }
    }
    result
}

#[derive(Clone, Debug)]
pub enum YamlFile {
    Embedded(&'static str),
    StaticPath(&'static str),
    DynamicPath(String),
}

impl YamlFile {
    fn load_into<T: TryFrom<Yaml, Error = anyhow::Error> + Hash + Eq>(
        &self,
        key: &str,
        dest: &mut HashSet<T>,
    ) -> Result<()> {
        match self {
            Self::Embedded(yaml) => load_from_yaml_array_str(yaml, key, dest),
            Self::StaticPath(path) => load_from_yaml_array_file(path, key, dest),
            Self::DynamicPath(path) => load_from_yaml_array_file(path, key, dest),
        }
    }
}

pub fn load_from_yaml_array_file<
    P: AsRef<Path>,
    T: TryFrom<Yaml, Error = anyhow::Error> + Hash + Eq,
>(
    path: P,
    key: &str,
    dest: &mut HashSet<T>,
) -> Result<()> {
    let yaml_str = fs::read_to_string(path.as_ref())?;
    load_from_yaml_array_str(&yaml_str, key, dest)
}

pub fn load_from_yaml_array_str<T: TryFrom<Yaml, Error = anyhow::Error> + Hash + Eq>(
    yaml_str: &str,
    key: &str,
    dest: &mut HashSet<T>,
) -> Result<()> {
    let docs = YamlLoader::load_from_str(yaml_str)?;
    docs.into_iter()
        .next()
        .ok_or(anyhow!("Empty yaml file"))?
        .into_hash()
        .ok_or(anyhow!("Expected top-level element to be a hash"))?
        .remove(&Yaml::String(key.into()))
        .ok_or(anyhow!("Missing top-level key {}", key))?
        .into_iter()
        .try_for_each(|yaml| {
            yaml.try_into().map(|t| {
                dest.insert(t);
            })
        })
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
