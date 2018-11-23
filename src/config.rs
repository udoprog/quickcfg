//! Model for configuration file.
use crate::{system::System, template::Template};
use serde::{Deserialize, Deserializer};
use serde_derive::Deserialize;
use std::time::Duration;

/// Default git refresh in seconds.
const DEFAULT_GIT_REFRESH_SECONDS: u64 = 3600 * 24 * 3;
/// Refresh package state every hour, unless changed.
const DEFAULT_PACKAGE_REFRESH_SECONDS: u64 = 3600;

/// Configuration model.
#[derive(Deserialize, Default, Debug, PartialEq, Eq)]
pub struct Config {
    /// The interval at which we check for git refresh.
    #[serde(
        default = "default_git_refresh",
        deserialize_with = "human_duration"
    )]
    pub git_refresh: Duration,

    /// The interval at which we check for packages.
    #[serde(
        default = "default_package_refresh",
        deserialize_with = "human_duration"
    )]
    pub package_refresh: Duration,

    /// The hierarchy at which we load `Data` from.
    pub hierarchy: Vec<Template>,
    /// The systems to apply.
    pub systems: Vec<System>,
}

/// Return default git refresh in seconds.
fn default_git_refresh() -> Duration {
    Duration::from_secs(DEFAULT_GIT_REFRESH_SECONDS)
}

/// Return default package refresh in seconds.
fn default_package_refresh() -> Duration {
    Duration::from_secs(DEFAULT_PACKAGE_REFRESH_SECONDS)
}

/// Parse a human duration.
pub fn human_duration<'de, D>(deserializer: D) -> Result<Duration, D::Error>
where
    D: Deserializer<'de>,
{
    let string = String::deserialize(deserializer)?;
    humantime::parse_duration(&string).map_err(serde::de::Error::custom)
}
