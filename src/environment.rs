//! Helpers for accessing environment variables.
use failure::{bail, Error};
use std::collections::HashMap;
use std::env;

pub trait Environment: Copy {
    /// Access the given environment variables.
    fn var(&self, key: &str) -> Result<Option<String>, Error>;
}

/// The real environment.
#[derive(Clone, Copy)]
pub struct Real;

impl Environment for Real {
    fn var(&self, key: &str) -> Result<Option<String>, Error> {
        let out = match env::var(key) {
            Ok(value) => Some(value),
            Err(env::VarError::NotPresent) => None,
            Err(e) => bail!("failed to get environment `{}`: {}", key, e),
        };

        Ok(out)
    }
}

/// A custom environment.
impl Environment for &HashMap<String, String> {
    fn var(&self, key: &str) -> Result<Option<String>, Error> {
        Ok(self.get(key).map(|s| s.to_string()))
    }
}
