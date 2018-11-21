//! Package abstraction.
//!
//! Can check which packages are installed.

mod debian;

use failure::Error;
use std::ffi::OsStr;

/// Package abstraction.
#[derive(Debug)]
pub enum Packages {
    Debian(debian::Packages),
}

/// Information about an installed package.
pub struct Package {
    pub name: String,
}

impl Packages {
    /// Detect which package provider to use.
    pub fn detect() -> Result<Option<Packages>, Error> {
        Ok(Some(Packages::Debian(debian::Packages::new())))
    }

    /// List all packages on this system.
    pub fn list_packages(&self) -> Result<Vec<Package>, Error> {
        match *self {
            Packages::Debian(ref p) => p.list_packages(),
        }
    }

    /// Install the given packages.
    pub fn install_packages<S>(&self, packages: impl IntoIterator<Item = S>) -> Result<(), Error>
    where
        S: AsRef<OsStr>,
    {
        match *self {
            Packages::Debian(ref p) => p.install_packages(packages),
        }
    }
}
