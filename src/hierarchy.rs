//! Dealing with the hierarchy of data.
use crate::{environment as e, facts::Facts, Template};
use failure::{bail, format_err, Error};
use serde::Deserialize;
use serde_yaml::{Mapping, Value};
use std::env;
use std::fs::File;
use std::path::Path;

const HEADER: &'static str = "quickcfg:";

/// Wrapper for hierarchy data.
pub struct Data(Vec<Mapping>);

impl Data {
    /// Load the given key.
    pub fn load<'de, T>(&self, key: &str) -> Result<Option<T>, Error>
    where
        T: Deserialize<'de>,
    {
        let key = serde_yaml::Value::String(key.to_string());

        for m in &self.0 {
            if let Some(value) = m.get(&key) {
                return Ok(Some(T::deserialize(value.clone())?));
            }
        }

        Ok(None)
    }

    /// Load the given key, if it doesn't exist, use a default value.
    pub fn load_or_default<'de, T>(&self, key: &str) -> Result<T, Error>
    where
        T: Default + Deserialize<'de>,
    {
        self.load(key).map(|v| v.unwrap_or_else(T::default))
    }

    /// Load the given key, if it doesn't exist, use a default value.
    pub fn load_array<'de, T>(&self, key: &str) -> Result<Vec<T>, Error>
    where
        T: Deserialize<'de>,
    {
        let key = serde_yaml::Value::String(key.to_string());

        let mut out = Vec::new();

        for m in &self.0 {
            if let Some(value) = m.get(&key) {
                let values: Vec<T> = Deserialize::deserialize(value.clone())?;
                out.extend(values);
            }
        }

        Ok(out)
    }

    /// Load data based on a file spec.
    /// This is typically in the first couple of lines in a file.
    pub fn load_from_spec<'a>(&self, content: &'a str) -> Result<Mapping, Error> {
        let mut m = Mapping::default();

        // look at the first 5 lines.
        for line in content.split("\n").take(5) {
            let index = match line.find(HEADER) {
                None => continue,
                Some(index) => index,
            };

            let spec = &line[index + HEADER.len()..].trim();

            for part in spec.split(",") {
                let part = part.trim();

                if part.is_empty() {
                    continue;
                }

                let mut it = part.splitn(2, ":");

                let key = match it.next() {
                    Some(key) => key,
                    None => bail!("bad part in specification `{}`: missing key", part),
                };

                let value = match it.next() {
                    Some("array") => Value::Sequence(self.load_array::<Value>(key)?),
                    Some("env") => {
                        let value = match env::var(key) {
                            Ok(value) => value,
                            Err(e) => bail!("failed to load environment variable `{}`: {}", key, e),
                        };

                        Value::String(value)
                    }
                    None => self
                        .load::<Value>(key)?
                        .ok_or_else(|| format_err!("missing key `{}` in hierarchy", key))?,
                    Some(other) => {
                        bail!("bad part in specification `{}`: bad type `{}`", part, other);
                    }
                };

                m.insert(Value::String(key.to_string()), value);
            }

            break;
        }

        return Ok(m);
    }
}

/// Load a hierarchy.
pub fn load<'a>(
    it: impl IntoIterator<Item = &'a Template>,
    root: &Path,
    facts: &Facts,
    environment: impl Copy + e::Environment,
) -> Result<Data, Error> {
    let mut stages = Vec::new();

    for h in it {
        let path = match h.render_as_relative_path(facts, environment)? {
            None => continue,
            Some(path) => path,
        };

        let path = path.to_path(root);

        let map = load_mapping(&path)
            .map_err(|e| format_err!("failed to load: {}: {}", path.display(), e))?;

        stages.push(map);
    }

    return Ok(Data(stages));

    /// Extend the existing mapping from the given hierarchy.
    fn load_mapping(path: &Path) -> Result<serde_yaml::Mapping, Error> {
        use serde_yaml::Value;

        let file = match File::open(&path) {
            Ok(file) => file,
            Err(e) => match e.kind() {
                _ => bail!("failed to open file: {}", e),
            },
        };

        match serde_yaml::from_reader(file)? {
            Value::Mapping(m) => return Ok(m),
            _ => bail!("exists, but is not a mapping"),
        }
    }
}
