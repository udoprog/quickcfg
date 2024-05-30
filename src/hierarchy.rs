//! Dealing with the hierarchy of data.

use anyhow::{anyhow, bail, Result};
use serde::Deserialize;
use serde_yaml::{Mapping, Value};
use std::env;
use std::fs::File;
use std::io;
use std::path::Path;
use std::time::SystemTime;

use crate::{environment as e, facts::Facts, Template};

const HEADER: &str = "quickcfg:";

/// Wrapper for hierarchy data.
pub struct Data {
    /// The last modification timestamp for a file in the hierarchy.
    pub last_modified: Option<SystemTime>,
    /// The hierarchy with data.
    hierarchy: Vec<Mapping>,
}

impl Data {
    /// Construct a new set of hierarchical data.
    pub fn new(last_modified: Option<SystemTime>, data: impl IntoIterator<Item = Mapping>) -> Self {
        Data {
            last_modified,
            hierarchy: data.into_iter().collect(),
        }
    }

    /// Load the given key.
    pub fn load<'de, T>(&self, key: &str) -> Result<Option<T>>
    where
        T: Deserialize<'de>,
    {
        let key = serde_yaml::Value::String(key.to_string());

        for m in &self.hierarchy {
            if let Some(value) = m.get(&key) {
                return Ok(Some(T::deserialize(value.clone())?));
            }
        }

        Ok(None)
    }

    /// Load the given key, if it doesn't exist, use a default value.
    pub fn load_or_default<'de, T>(&self, key: &str) -> Result<T>
    where
        T: Default + Deserialize<'de>,
    {
        self.load(key).map(|v| v.unwrap_or_default())
    }

    /// Load the given key, if it doesn't exist, use a default value.
    pub fn load_array<'de, T>(&self, key: &str) -> Result<Vec<T>>
    where
        T: Deserialize<'de>,
    {
        let key = serde_yaml::Value::String(key.to_string());

        let mut out = Vec::new();

        for m in &self.hierarchy {
            if let Some(value) = m.get(&key) {
                out.extend(<Vec<T> as Deserialize>::deserialize(value.clone())?);
            }
        }

        Ok(out)
    }

    /// Load data based on a file spec.
    /// This is typically in the first couple of lines in a file.
    pub fn load_from_spec(&self, content: &str) -> Result<Mapping> {
        let mut m = Mapping::default();

        // look at the first 5 lines.
        for line in content.split('\n').take(5) {
            let index = match line.find(HEADER) {
                None => continue,
                Some(index) => index,
            };

            let spec = &line[index + HEADER.len()..].trim();

            for part in spec.split(',') {
                let part = part.trim();

                if part.is_empty() {
                    continue;
                }

                let mut it = part.splitn(2, ':');

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
                        .ok_or_else(|| anyhow!("missing key `{}` in hierarchy", key))?,
                    Some(other) => {
                        bail!("bad part in specification `{}`: bad type `{}`", part, other);
                    }
                };

                m.insert(Value::String(key.to_string()), value);
            }

            break;
        }

        Ok(m)
    }
}

/// Load a hierarchy.
pub fn load<'a>(
    it: impl IntoIterator<Item = &'a Template>,
    root: &Path,
    facts: &Facts,
    environment: impl e::Environment,
) -> Result<Data> {
    let mut stages = Vec::new();
    let mut last_modified = None;

    for h in it {
        let path = match h.as_relative_path(facts, environment)? {
            None => continue,
            Some(path) => path,
        };

        let path = path.to_path(root);

        let m = match path.metadata() {
            Ok(m) => m,
            Err(e) => match e.kind() {
                io::ErrorKind::NotFound => {
                    log::trace!("skipping missing file: {}", path.display());
                    continue;
                }
                _ => return Err(anyhow::Error::from(e)),
            },
        };

        let modified = m.modified()?;

        last_modified = Some(match last_modified {
            Some(previous) if previous > modified => previous,
            _ => modified,
        });

        let map = load_mapping(&path)
            .map_err(|e| anyhow!("failed to load: {}: {}", path.display(), e))?;

        stages.push(map);
    }

    return Ok(Data::new(last_modified, stages));

    /// Extend the existing mapping from the given hierarchy.
    fn load_mapping(path: &Path) -> Result<serde_yaml::Mapping> {
        let file = match File::open(path) {
            Ok(file) => file,
            Err(e) => bail!("failed to open file: {e}"),
        };

        match serde_yaml::from_reader(file)? {
            Value::Mapping(m) => Ok(m),
            _ => bail!("exists, but is not a mapping"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Data;
    use serde_yaml::{Mapping, Value};

    #[test]
    fn test_hiera_lookup() {
        let mut layer1 = Mapping::new();
        let mut layer2 = Mapping::new();

        layer1.insert("foo".into(), "foo value".into());
        layer1.insert("seq".into(), vec![Value::from("item1")].into());
        layer2.insert("bar".into(), "bar value".into());
        layer2.insert("seq".into(), vec![Value::from("item2")].into());

        let data = Data::new(None, vec![layer1, layer2]);

        assert_eq!(
            data.load::<String>("foo").expect("layer1 key as string"),
            Some("foo value".into()),
        );

        assert_eq!(
            data.load::<String>("bar").expect("layer2 key as string"),
            Some("bar value".into()),
        );

        assert_eq!(
            data.load_or_default::<String>("missing")
                .expect("missing key to default"),
            "",
        );

        assert_eq!(
            data.load_array::<String>("seq")
                .expect("merged array from layers"),
            vec![String::from("item1"), String::from("item2")],
        );
    }
}
