//! Loading facts about the system that we are currently running on.

use std::collections::HashMap;
use failure::{bail, Error};
use std::path::Path;
use std::env;
use std::fs;
use std::io;

/// The holder of all the facts detected in the system.
pub struct Facts(pub HashMap<String, String>);

/// Load facts.
pub fn load() -> Result<Facts, Error> {
    let mut facts = HashMap::new();

    if let Some(distro) = detect_distro()? {
        facts.insert("distro".to_string(), distro);
    }

    return Ok(Facts(facts));

    /// Detect which distro we appear to be running.
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

        if environ("OSTYPE")?
            .map(|s| s.starts_with("darwin"))
            .unwrap_or(false)
        {
            return Ok(Some("osx".to_string()));
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

    fn environ(key: &str) -> Result<Option<String>, Error> {
        let value = match env::var(key) {
            Ok(value) => value,
            Err(env::VarError::NotPresent) => return Ok(None),
            Err(e) => bail!("failed to load environment var: {}: {}", key, e),
        };

        Ok(Some(value))
    }
}

