//! Loading facts about the system that we are currently running on.

use crate::template::Vars;
use anyhow::{Error, bail};
use std::borrow::Borrow;
use std::collections::HashMap;
use std::fs;
use std::hash::Hash;
use std::io;
use std::path::Path;

/// The `distro` fact key.
pub const DISTRO: &str = "distro";

/// The `os` fact key.
pub const OS: &str = "os";

/// The holder of all the facts detected in the system.
pub struct Facts(HashMap<String, String>);

impl Facts {
    /// Construct a set of custom facts.
    pub fn new(facts: impl IntoIterator<Item = (String, String)>) -> Self {
        Facts(facts.into_iter().collect())
    }

    /// Load facts about the system.
    pub fn load() -> Result<Facts, Error> {
        let mut facts = HashMap::new();

        if let Some(distro) = detect_distro()? {
            facts.insert(DISTRO.to_string(), distro);
        }

        facts.insert(OS.to_string(), std::env::consts::OS.to_string());
        return Ok(Facts(facts));

        /// Detect which distro we appear to be running.
        #[allow(unreachable_code)]
        fn detect_distro() -> Result<Option<String>, Error> {
            if metadata("/etc/redhat-release")?
                .map(|m| m.is_file())
                .unwrap_or(false)
            {
                return Ok(Some("fedora".to_string()));
            }

            if metadata("/etc/gentoo-release")?
                .map(|m| m.is_file())
                .unwrap_or(false)
            {
                return Ok(Some("gentoo".to_string()));
            }

            if metadata("/etc/debian_version")?
                .map(|m| m.is_file())
                .unwrap_or(false)
            {
                return Ok(Some("debian".to_string()));
            }

            Ok(None)
        }

        fn metadata<P: AsRef<Path>>(path: P) -> Result<Option<fs::Metadata>, Error> {
            let p = path.as_ref();

            let m = match fs::metadata(p) {
                Ok(m) => m,
                Err(e) => match e.kind() {
                    io::ErrorKind::NotFound => return Ok(None),
                    _ => bail!("failed to load file metadata: {}: {}", p.display(), e),
                },
            };

            Ok(Some(m))
        }
    }

    /// Get the specified fact, if present.
    pub fn get<Q>(&self, k: &Q) -> Option<&str>
    where
        String: Borrow<Q>,
        Q: ?Sized + Hash + Eq,
    {
        self.0.get(k).map(|s| s.as_str())
    }
}

impl Vars for &Facts {
    fn get(&self, k: &str) -> Option<&str> {
        Facts::get(self, k)
    }
}
