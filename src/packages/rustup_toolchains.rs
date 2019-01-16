//! Packages abstraction for rustup toolchains.

use crate::{packages::Package, rustup};
use failure::Error;

/// Packages abstraction for rustup toolchains.
#[derive(Debug)]
pub struct PackageManager {
    rustup: rustup::Rustup,
}

impl PackageManager {
    /// Construct a new rustup package manager.
    pub fn new() -> Self {
        PackageManager {
            rustup: rustup::Rustup::new("toolchain", "install"),
        }
    }
}

impl super::PackageManager for PackageManager {
    fn primary(&self) -> bool {
        true
    }

    fn name(&self) -> &str {
        "rust toolchains"
    }

    fn key(&self) -> Option<&str> {
        Some("rust::toolchains")
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
