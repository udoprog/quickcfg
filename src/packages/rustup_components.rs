//! Packages abstraction for rustup components.

use crate::{packages::Package, rustup};
use failure::Error;

/// Packages abstraction for rustup components.
#[derive(Debug)]
pub struct PackageManager {
    rustup: rustup::Rustup,
}

impl PackageManager {
    /// Construct a new rustup package manager.
    pub fn new() -> Self {
        PackageManager {
            rustup: rustup::Rustup::new("component", "add"),
        }
    }
}

impl super::PackageManager for PackageManager {
    fn primary(&self) -> bool {
        true
    }

    fn name(&self) -> &str {
        "rust components"
    }

    fn key(&self) -> Option<&str> {
        Some("rust::components")
    }

    /// Test that we have everything we need.
    fn test(&self) -> Result<bool, Error> {
        self.rustup.test()
    }

    fn list_packages(&self) -> Result<Vec<Package>, Error> {
        self.rustup.list_installed()
    }

    fn install_packages(&self, packages: &[String]) -> Result<(), Error> {
        self.rustup.install_packages(packages)
    }
}
