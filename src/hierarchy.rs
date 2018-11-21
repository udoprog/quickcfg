//! Dealing with the hierarchy of data.
use crate::{
    facts::Facts,
    environment as e, Template
};
use failure::{bail, format_err, Error};
use std::path::Path;
use std::fs::File;
use std::io;
use log::info;
use serde::Deserialize;

/// Wrapper for hierarchy data.
pub struct Data(pub serde_yaml::Mapping);

impl Data {
    /// Load the given key, if it doesn't exist, use a default value.
    pub fn load_or_default<'de, T: Default>(&self, key: &str) -> Result<T, Error> where T: Deserialize<'de> {
        match self.0.get(&serde_yaml::Value::String(key.to_string())) {
            None => Ok(T::default()),
            Some(value) => Ok(T::deserialize(value.clone())?),
        }
    }
}

/// Load a hierarchy.
pub fn load<'a>(
    it: impl IntoIterator<Item = &'a Template>,
    root: &Path,
    facts: &Facts,
    environment: impl Copy + e::Environment,
) -> Result<Data, Error> {
    use serde_yaml::{Mapping, Value};

    let mut map = Mapping::new();

    for h in it {
        let path = match h.render_as_relative_path(facts, environment)? {
            None => continue,
            Some(path) => path,
        };

        let path = path.to_path(root);

        extend_from_hierarchy(&mut map, &path).map_err(|e| {
            format_err!("failed to extend hierarchy from: {}: {}", path.display(), e)
        })?;
    }

    return Ok(Data(map));

    /// Extend the existing mapping from the given hierarchy.
    fn extend_from_hierarchy(map: &mut Mapping, path: &Path) -> Result<(), Error> {
        let file = match File::open(&path) {
            Ok(file) => file,
            Err(e) => match e.kind() {
                io::ErrorKind::NotFound => return Ok(()),
                _ => bail!("failed to open file: {}", e),
            },
        };

        match serde_yaml::from_reader(file)? {
            Value::Mapping(m) => {
                map.extend(m);
            }
            other => {
                bail!(
                    "Cannot deal with value `{:?}` from hierarchy: {}",
                    other,
                    path.display()
                );
            }
        }

        info!("LOAD PATH: {}", path.display());
        Ok(())
    }
}
