//! Package abstraction.
//!
//! Can check which packages are installed.

mod debian;

use crate::facts::{self, Facts};
use failure::Error;
use fxhash::FxHashMap;
use log::warn;
use std::sync::Arc;

/// Information about an installed package.
pub struct Package {
    pub name: String,
}

/// A provider of package managers.
pub struct Provider {
    by_name: FxHashMap<String, Arc<dyn PackageManager>>,
    default: Option<Arc<dyn PackageManager>>,
}

impl Provider {
    /// Access the default package manager if it is available.
    pub fn default(&self) -> Option<&dyn PackageManager> {
        self.default.as_ref().map(|p| &**p)
    }

    /// Look up a package manager by name.
    pub fn get(&self, name: &str) -> Option<&dyn PackageManager> {
        self.by_name.get(name).map(|p| &**p)
    }
}

/// Detect which package provider to use.
pub fn detect(facts: &Facts) -> Result<Provider, Error> {
    let mut managers = Vec::new();
    managers.extend(by_distro(facts)?);

    let by_name = managers
        .iter()
        .map(|p| (p.name().to_string(), Arc::clone(p)))
        .collect::<FxHashMap<_, _>>();
    let default = managers
        .iter()
        .filter(|p| p.primary())
        .map(|p| Arc::clone(p))
        .next();

    Ok(Provider { by_name, default })
}

/// Detect package manager by distro.
fn by_distro(facts: &Facts) -> Result<Option<Arc<dyn PackageManager>>, Error> {
    let distro = match facts.get(facts::DISTRO) {
        // NB: unsupported distro, good luck!
        None => return Ok(None),
        Some(distro) => distro,
    };

    match distro {
        "debian" => Ok(Some(Arc::new(debian::PackageManager::new()))),
        distro => {
            warn!("no package integration for distro: {}", distro);
            Ok(None)
        }
    }
}

/// The trait that describes a package manager.
pub trait PackageManager: Sync + Send {
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
