//! Packages abstraction for Cargo.

use crate::{command, os, packages::Package};
use anyhow::{anyhow, Error};
use std::ffi::OsStr;
use std::io;

#[derive(Debug)]
pub struct Cargo {
    cargo: command::Command<'static>,
}

impl Cargo {
    /// Create a new cargo command wrapper.
    pub fn new() -> Self {
        Cargo {
            cargo: command::Command::new(os::command("cargo")),
        }
    }

    /// Test that the command is available.
    pub fn test(&self) -> Result<bool, Error> {
        match self.cargo.run(&["--version"]) {
            Ok(output) => Ok(output.status.success()),
            Err(e) => match e.kind() {
                // no such command.
                io::ErrorKind::NotFound => Ok(false),
                _ => Err(Error::from(e)),
            },
        }
    }

    /// List all the packages which are installed.
    pub fn install_packages<S>(&self, packages: impl IntoIterator<Item = S>) -> Result<(), Error>
    where
        S: AsRef<OsStr>,
    {
        let packages = packages.into_iter().collect::<Vec<_>>();

        let mut args = Vec::new();
        args.push(OsStr::new("install"));
        args.extend(packages.iter().map(AsRef::as_ref));

        self.cargo.run(args)?;
        Ok(())
    }

    /// List all the packages which are installed.
    pub fn list_installed(&self) -> Result<Vec<Package>, Error> {
        let mut out = Vec::new();

        for line in self.cargo.run_lines(&["install", "--list"])? {
            if line.starts_with(char::is_whitespace) {
                continue;
            }

            let line = line.trim();

            if line == "" {
                continue;
            }

            let mut it = line.split(' ');

            let name = it.next().ok_or_else(|| anyhow!("expected package name"))?;

            out.push(Package {
                name: name.to_string(),
            });
        }

        Ok(out)
    }
}

/// Packages abstraction for Cargo.
#[derive(Debug)]
pub struct PackageManager {
    cargo: Cargo,
}

impl PackageManager {
    /// Construct a new cargo package manager.
    pub fn new() -> Self {
        PackageManager {
            cargo: Cargo::new(),
        }
    }
}

impl super::PackageManager for PackageManager {
    fn primary(&self) -> bool {
        true
    }

    fn name(&self) -> &str {
        "cargo"
    }

    /// Test that we have everything we need.
    fn test(&self) -> Result<bool, Error> {
        self.cargo.test()
    }

    fn list_packages(&self) -> Result<Vec<Package>, Error> {
        self.cargo.list_installed()
    }

    fn install_packages(&self, packages: &[String]) -> Result<(), Error> {
        self.cargo.install_packages(packages)
    }
}
