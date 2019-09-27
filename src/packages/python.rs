//! Packages abstraction for pip/pip3.

use crate::{command, os, packages::Package};
use failure::{format_err, Error};
use std::ffi::OsStr;
use std::io;

#[derive(Debug)]
pub struct Pip {
    command: command::Command<'static>,
}

impl Pip {
    /// Create a new pip command wrapper.
    pub fn new(name: &'static str) -> Self {
        Pip {
            command: command::Command::new(os::command(name)),
        }
    }

    /// Test that the command is available.
    pub fn test(&self) -> Result<bool, Error> {
        match self.command.run(&["--version"]) {
            Ok(output) => Ok(output.status.success()),
            Err(e) => match e.kind() {
                // no such command.
                io::ErrorKind::NotFound => Ok(false),
                _ => Err(Error::from(e)),
            },
        }
    }

    /// List all the packages which are installed.
    pub fn list_installed(&self) -> Result<Vec<Package>, Error> {
        let mut out = Vec::new();

        let args = &["list", "--format=legacy"];

        for line in self.command.run_lines(args)? {
            let line = line.trim();

            if line == "" {
                continue;
            }

            let mut it = line.split(' ');
            let name = it
                .next()
                .ok_or_else(|| format_err!("expected package name"))?;

            out.push(Package {
                name: name.to_string(),
            });
        }

        Ok(out)
    }

    /// List all the packages which are installed.
    pub fn install_packages<S>(&self, packages: impl IntoIterator<Item = S>) -> Result<(), Error>
    where
        S: AsRef<OsStr>,
    {
        let packages = packages.into_iter().collect::<Vec<_>>();

        let mut args = Vec::new();
        args.push(OsStr::new("install"));
        args.push(OsStr::new("--user"));
        args.extend(packages.iter().map(AsRef::as_ref));

        self.command.run(args)?;
        Ok(())
    }
}

/// Packages abstraction for pip.
#[derive(Debug)]
pub struct PackageManager {
    name: &'static str,
    pip: Pip,
}

impl PackageManager {
    /// Construct a new pip package manager.
    pub fn new(name: &'static str) -> Self {
        PackageManager {
            name,
            pip: Pip::new(name),
        }
    }
}

impl super::PackageManager for PackageManager {
    fn primary(&self) -> bool {
        true
    }

    fn name(&self) -> &str {
        self.name
    }

    /// Test if command is available.
    fn test(&self) -> Result<bool, Error> {
        self.pip.test()
    }

    fn list_packages(&self) -> Result<Vec<Package>, Error> {
        self.pip.list_installed()
    }

    fn install_packages(&self, packages: &[String]) -> Result<(), Error> {
        self.pip.install_packages(packages)
    }
}
