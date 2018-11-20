//! Model for configuration file.
use crate::{system::System, template::Template};
use relative_path::RelativePathBuf;
use serde_derive::Deserialize;

/// Configuration model.
#[derive(Deserialize, Default, Debug, PartialEq, Eq)]
pub struct Config {
    pub home: Option<RelativePathBuf>,
    pub hierarchy: Vec<Template>,
    pub systems: Vec<System>,
}
