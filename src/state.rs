//! Model for state file.

use crate::config::Config;
use crate::Timestamp;
use anyhow::Error;
use fxhash::FxHasher64;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};

#[derive(Deserialize, Serialize, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Hashed {
    /// The last calculated hash.
    pub hash: u64,
    /// When it was last updated.
    pub updated: Timestamp,
}

/// The way the state is serialized.
#[derive(Deserialize, Serialize, Default, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DiskState {
    /// Last time git was updated.
    #[serde(default)]
    pub last_update: BTreeMap<String, Timestamp>,
    /// Things that should only happen once.
    #[serde(default)]
    pub once: BTreeMap<String, Timestamp>,
    #[serde(default)]
    pub hashes: BTreeMap<String, Hashed>,
}

impl DiskState {
    /// Convert into a state.
    pub fn into_state(self, config: &Config, now: Timestamp) -> State<'_> {
        State {
            dirty: false,
            last_update: self.last_update,
            once: self.once,
            hashes: self.hashes,
            config,
            now,
        }
    }
}

/// State model.
/// This keeps track of any changes with the dirty flag, which is an indication whether it should
/// be serialized or not.
#[derive(Debug, PartialEq, Eq)]
pub struct State<'a> {
    pub dirty: bool,
    /// Last time git was updated.
    pub last_update: BTreeMap<String, Timestamp>,
    /// Things that should only happen once.
    pub once: BTreeMap<String, Timestamp>,
    /// Things that have been tested against a hash.
    pub hashes: BTreeMap<String, Hashed>,
    /// The current configuration.
    pub config: &'a Config,
    /// Current timestamp.
    pub now: Timestamp,
}

impl<'a> State<'a> {
    pub fn new(config: &'a Config, now: Timestamp) -> Self {
        State {
            dirty: Default::default(),
            last_update: Default::default(),
            once: Default::default(),
            hashes: Default::default(),
            config,
            now,
        }
    }

    /// Get the last update timestamp for the given thing named `name`.
    pub fn last_update<'time>(&'time self, name: &str) -> Option<&'time Timestamp> {
        self.last_update.get(name)
    }

    /// Touch the thing with the given name.
    pub fn touch(&mut self, name: &str) {
        self.dirty = true;
        self.last_update.insert(name.to_string(), Timestamp::now());
    }

    /// Check if the given ID has run once.
    pub fn has_run_once(&self, id: &str) -> bool {
        self.once.contains_key(id)
    }

    /// Mark that something has happened once.
    pub fn touch_once(&mut self, id: &str) {
        self.dirty = true;
        self.once.insert(id.to_string(), Timestamp::now());
    }

    /// Touch the hashed item.
    pub fn is_hash_fresh<H: Hash>(&self, id: &str, hash: H) -> Result<bool, Error> {
        let hashed = match self.hashes.get(id) {
            Some(hashed) => hashed,
            None => return Ok(false),
        };

        let mut state = FxHasher64::default();
        hash.hash(&mut state);

        if hashed.hash != state.finish() {
            return Ok(false);
        }

        let age = self.now.duration_since(hashed.updated)?;
        Ok(age < self.config.package_refresh)
    }

    /// Touch the hashed item.
    pub fn touch_hash<H: Hash>(&mut self, id: &str, hash: H) -> Result<(), Error> {
        let mut state = FxHasher64::default();
        hash.hash(&mut state);

        self.dirty = true;

        self.hashes.insert(
            id.to_string(),
            Hashed {
                hash: state.finish(),
                updated: Timestamp::now(),
            },
        );

        Ok(())
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
        self.hashes.extend(other.hashes);
    }

    /// Serialize the state, returning `None` unless it is dirty.
    pub fn serialize(self) -> Option<DiskState> {
        if !self.dirty {
            return None;
        }

        Some(DiskState {
            last_update: self.last_update,
            once: self.once,
            hashes: self.hashes,
        })
    }
}
