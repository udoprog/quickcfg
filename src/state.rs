//! Model for state file.

use serde_derive::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::time::SystemTime;

/// State model.
#[derive(Deserialize, Serialize, Default, Debug, PartialEq, Eq)]
pub struct State {
    /// Last time git was updated.
    pub last_update: BTreeMap<String, SystemTime>,
}

impl State {
    /// Get the last update timestamp for the given thing named `name`.
    pub fn last_update<'a>(&'a self, name: &str) -> Option<&'a SystemTime> {
        self.last_update.get(name)
    }

    /// Touch the thing with the given name.
    pub fn touch(&mut self, name: &str) {
        self.last_update.insert(name.to_string(), SystemTime::now());
    }
}
