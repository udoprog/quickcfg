//! Package abstraction.
//!
//! Can check which packages are installed.

mod debian;
mod python;

use crate::facts::{self, Facts};
use failure::{bail, Error};
use log::warn;
use std::fmt;
use std::sync::Arc;

/// Information about an installed package.
pub struct Package {
    pub name: String,
}

/// A provider of package managers.
pub struct Provider {
    default: Option<Arc<dyn PackageManager>>,
}

impl Provider {
    /// Access the default package manager if it is available.
    pub fn default(&self) -> Option<Arc<dyn PackageManager>> {
        self.default.as_ref().map(Arc::clone)
    }

    /// Look up a package manager by name.
    pub fn get(&self, name: &str) -> Result<Option<Arc<dyn PackageManager>>, Error> {
        if let Some(default) = self.default.as_ref() {
            if default.name() == name {
                return Ok(Some(Arc::clone(default)));
            }
        }

        match name {
            "debian" => test_debian(),
            "pip" => test_python("pip"),
            "pip3" => test_python("pip3"),
            _ => bail!("No package manager provider for `{}`", name),
        }
    }
}

/// Detect which package provider to use.
pub fn detect(facts: &Facts) -> Result<Provider, Error> {
    let default = by_distro(facts)?;
    return Ok(Provider { default });
}

/// Detect package manager by distro.
fn by_distro(facts: &Facts) -> Result<Option<Arc<dyn PackageManager>>, Error> {
    let distro = match facts.get(facts::DISTRO) {
        // NB: unsupported distro, good luck!
        None => return Ok(None),
        Some(distro) => distro,
    };

    match distro {
        "debian" => test_debian(),
        distro => {
            warn!("no package integration for distro: {}", distro);
            Ok(None)
        }
    }
}

/// Verify that we have access to everything we need for debian.
fn test_debian() -> Result<Option<Arc<dyn PackageManager>>, Error> {
    let debian = debian::PackageManager::new();

    if !debian.test()? {
        bail!("Not a supported Debian environment");
    }

    Ok(Some(Arc::new(debian)))
}

/// Try to detect existing python package managers.
fn test_python(name: &'static str) -> Result<Option<Arc<dyn PackageManager>>, Error> {
    let pip = python::PackageManager::new(name);

    if pip.test()? {
        Ok(Some(Arc::new(pip)))
    } else {
        Ok(None)
    }
}

/// The trait that describes a package manager.
pub trait PackageManager: fmt::Debug + Sync + Send {
    /// Is this a primary package manager?
    fn primary(&self) -> bool {
        false
    }

    /// Get the name of the current package manager.
    fn name(&self) -> &str;

    /// List all packages on this system.
    fn list_packages(&self) -> Result<Vec<Package>, Error>;

    /// Install the given packages.
    fn install_packages(&self, packages: &[String]) -> Result<(), Error>;
}
