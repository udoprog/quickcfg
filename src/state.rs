//! Model for state file.

use serde_derive::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::time::SystemTime;

/// The way the state is serialized.
#[derive(Deserialize, Serialize, Default, Debug, PartialEq, Eq)]
pub struct DiskState {
    /// Last time git was updated.
    pub last_update: BTreeMap<String, SystemTime>,
}

impl DiskState {
    /// Convert into a state.
    pub fn to_state(self) -> State {
        State {
            dirty: false,
            last_update: self.last_update,
        }
    }
}

/// State model.
/// This keeps track of any changes with the dirty flag, which is an indication whether it should
/// be serialized or not.
#[derive(Default, Debug, PartialEq, Eq)]
pub struct State {
    pub dirty: bool,
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
        self.dirty = true;
        self.last_update.insert(name.to_string(), SystemTime::now());
    }

    /// Serialize the state, returning `None` unless it is dirty.
    pub fn serialize(self) -> Option<DiskState> {
        if !self.dirty {
            return None;
        }

        Some(DiskState {
            last_update: self.last_update,
        })
    }
}
