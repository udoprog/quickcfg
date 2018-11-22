//! Package abstraction.
//!
//! Can check which packages are installed.

mod debian;
mod python;

use crate::facts::{self, Facts};
use failure::Error;
use fxhash::FxHashMap;
use log::warn;
use std::fmt;
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
    pub fn default(&self) -> Option<&Arc<dyn PackageManager>> {
        self.default.as_ref()
    }

    /// Look up a package manager by name.
    pub fn get(&self, name: &str) -> Option<&Arc<dyn PackageManager>> {
        self.by_name.get(name)
    }
}

/// Detect which package provider to use.
pub fn detect(facts: &Facts) -> Result<Provider, Error> {
    use rayon::prelude::*;

    let mut tests: Vec<Test> = Vec::new();

    // The various tests to perform.
    tests.push(Test::Distro(facts));
    tests.push(Test::Python("pip"));
    tests.push(Test::Python("pip3"));

    // Test in parallel.
    let managers = tests
        .into_par_iter()
        .map(Test::apply)
        .flat_map(|res| match res {
            Ok(Some(p)) => Some(Ok(p)),
            Ok(None) => None,
            Err(e) => Some(Err(e)),
        }).collect::<Result<Vec<_>, Error>>()?;

    let by_name = managers
        .iter()
        .map(|p| (p.name().to_string(), Arc::clone(p)))
        .collect::<FxHashMap<_, _>>();

    let default = managers
        .iter()
        .filter(|p| p.primary())
        .map(|p| Arc::clone(p))
        .next();

    return Ok(Provider { by_name, default });

    // A test for a package manager that can run in parallel.
    pub enum Test<'a> {
        Distro(&'a Facts),
        Python(&'static str),
    }

    impl<'a> Test<'a> {
        /// Apply the test.
        fn apply(self) -> Result<Option<Arc<dyn PackageManager>>, Error> {
            match self {
                Test::Distro(facts) => by_distro(facts),
                Test::Python(name) => test_python(name),
            }
        }
    }
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
