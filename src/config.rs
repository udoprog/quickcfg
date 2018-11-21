//! Model for configuration file.
use crate::{system::System, template::Template};
use failure::{bail, format_err, Error};
use relative_path::RelativePathBuf;
use serde_derive::Deserialize;
use serde_yaml;
use std::fs::File;
use std::io;
use std::path::Path;

/// Configuration model.
#[derive(Deserialize, Default, Debug, PartialEq, Eq)]
pub struct Config {
    pub home: Option<RelativePathBuf>,
    pub hierarchy: Vec<Template>,
    pub systems: Vec<System>,
}

impl Config {
    /// Load configuration from the given path.
    pub fn load(path: &Path) -> Result<Option<Config>, Error> {
        let f = match File::open(path) {
            Ok(f) => f,
            Err(e) => match e.kind() {
                io::ErrorKind::NotFound => return Ok(None),
                _ => bail!("could not open file: {}", e),
            },
        };

        let c = serde_yaml::from_reader(f).map_err(|e| format_err!("failed to parse: {}", e))?;

        Ok(c)
    }
}
