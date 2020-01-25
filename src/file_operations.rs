//! Utilities for reading and writing serde types to and from the filesystem.

use anyhow::{bail, format_err, Context as _, Error};
use serde::de::DeserializeOwned;
use serde::ser::Serialize;
use serde_yaml;
use std::fs::File;
use std::io;
use std::path::Path;

pub trait Load: Sized {
    /// Load the file from the given path.
    fn load(path: &Path) -> Result<Option<Self>, Error>;
}

pub trait Save {
    /// Save the state to the given file.
    fn save(&self, path: &Path) -> Result<(), Error>;
}

impl<T> Load for T
where
    T: DeserializeOwned,
{
    fn load(path: &Path) -> Result<Option<Self>, Error> {
        let f = match File::open(path) {
            Ok(f) => f,
            Err(e) => match e.kind() {
                io::ErrorKind::NotFound => return Ok(None),
                _ => bail!("Could not open file: {}", e),
            },
        };

        let out: T =
            serde_yaml::from_reader(f).with_context(|| format_err!("Failed to parse as YAML"))?;
        Ok(Some(out))
    }
}

impl<T> Save for T
where
    T: Serialize,
{
    fn save(&self, path: &Path) -> Result<(), Error> {
        let f = File::create(path).map_err(|e| format_err!("could not open file: {}", e))?;
        serde_yaml::to_writer(f, self).map_err(|e| format_err!("failed to write: {}", e))?;
        Ok(())
    }
}
