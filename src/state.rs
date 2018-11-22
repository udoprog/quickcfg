//! Model for state file.

use serde_derive::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::time::SystemTime;

/// The way the state is serialized.
#[derive(Deserialize, Serialize, Default, Debug, PartialEq, Eq)]
pub struct DiskState {
    /// Last time git was updated.
    #[serde(default)]
    pub last_update: BTreeMap<String, SystemTime>,
    /// Things that should only happen once.
    #[serde(default)]
    pub once: BTreeMap<String, SystemTime>,
}

impl DiskState {
    /// Convert into a state.
    pub fn to_state(self) -> State {
        State {
            dirty: false,
            last_update: self.last_update,
            once: self.once,
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
    /// Things that should only happen once.
    pub once: BTreeMap<String, SystemTime>,
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

    /// Check if the given ID has run once.
    pub fn has_run_once(&self, id: &str) -> bool {
        self.once.contains_key(id)
    }

    /// Mark that something has happened once.
    pub fn touch_once(&mut self, id: &str) {
        self.dirty = true;
        self.once.insert(id.to_string(), SystemTime::now());
    }

    /// Extend this state with another.
    pub fn extend(&mut self, other: State) {
        // nothing to extend.
        if !other.dirty {
            return;
        }

        self.dirty = true;
        self.last_update.extend(other.last_update);
        self.once.extend(other.once);
    }

    /// Serialize the state, returning `None` unless it is dirty.
    pub fn serialize(self) -> Option<DiskState> {
        if !self.dirty {
            return None;
        }

        Some(DiskState {
            last_update: self.last_update,
            once: self.once,
        })
    }
}
