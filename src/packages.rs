//! Package abstraction.
//!
//! Can check which packages are installed.

mod debian;

use failure::Error;

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
            Packages::Debian(ref packages) => packages.list_packages(),
        }
    }
}
