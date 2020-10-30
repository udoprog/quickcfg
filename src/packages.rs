//! Package abstraction.
//!
//! Can check which packages are installed.

mod cargo;
mod debian;
mod python;
mod ruby;
mod rustup_components;
mod rustup_toolchains;
mod winget;

use crate::facts::{self, Facts};
use anyhow::{bail, Error};
use log::warn;
use std::fmt;
use std::sync::Arc;

/// Information about an installed package.
#[derive(Debug)]
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
            "debian" => test(debian::PackageManager::new()),
            "pip" => test(python::PackageManager::new("pip")),
            "pip3" => test(python::PackageManager::new("pip3")),
            "gem" => test(ruby::PackageManager::new()),
            "cargo" => test(cargo::PackageManager::new()),
            "winget" => test(winget::PackageManager::new()),
            "rust toolchains" => test(rustup_toolchains::PackageManager::new()),
            "rust components" => test(rustup_components::PackageManager::new()),
            _ => bail!("No package manager provider for `{}`", name),
        }
    }
}

/// Detect which package provider to use.
pub fn detect(facts: &Facts) -> Result<Provider, Error> {
    let default = if let Some(default) = by_distro(facts)? {
        Some(default)
    } else if let Some(default) = by_os(facts)? {
        Some(default)
    } else {
        None
    };

    Ok(Provider { default })
}

/// Detect package manager by distro.
fn by_distro(facts: &Facts) -> Result<Option<Arc<dyn PackageManager>>, Error> {
    let distro = match facts.get(facts::DISTRO) {
        // NB: unsupported distro, good luck!
        None => return Ok(None),
        Some(distro) => distro,
    };

    match distro {
        "debian" => test(debian::PackageManager::new()),
        distro => {
            warn!("no package integration for distro: {}", distro);
            Ok(None)
        }
    }
}

/// Detect package manager by OS.
fn by_os(facts: &Facts) -> Result<Option<Arc<dyn PackageManager>>, Error> {
    let os = match facts.get(facts::OS) {
        // NB: unsupported os, good luck!
        None => return Ok(None),
        Some(os) => os,
    };

    match os {
        "windows" => test(winget::PackageManager::new()),
        os => {
            warn!("no package integration for os: {}", os);
            Ok(None)
        }
    }
}

/// Try to detect existing python package managers.
fn test(manager: impl PackageManager + 'static) -> Result<Option<Arc<dyn PackageManager>>, Error> {
    if manager.test()? {
        Ok(Some(Arc::new(manager)))
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

    /// Might this package manager require interaction?
    fn needs_interaction(&self) -> bool {
        false
    }

    /// Hierarchy key to use for package manager.
    fn key(&self) -> Option<&str> {
        None
    }

    /// Get the name of the current package manager.
    fn name(&self) -> &str;

    /// Test if package manager is usable.
    fn test(&self) -> Result<bool, Error>;

    /// List all packages on this system.
    fn list_packages(&self) -> Result<Vec<Package>, Error>;

    /// Install the given packages.
    fn install_packages(&self, packages: &[String]) -> Result<(), Error>;
}
