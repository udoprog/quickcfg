//! Packages abstraction for WinGet.

use crate::{command, os, packages::Package};
use anyhow::Error;
#[derive(Debug)]
pub struct WinGet {
    winget: command::Command,
}

impl WinGet {
    /// Create a new winget command wrapper.
    pub fn new() -> Self {
        Self {
            winget: command::Command::new(os::command("winget")),
        }
    }

    /// Test that the command is available.
    #[cfg(windows)]
    pub fn test(&self) -> Result<bool, Error> {
        use std::io;

        let mut winget = self.winget.clone();
        winget.arg("--version");

        match winget.run() {
            Ok(output) => Ok(output.status.success()),
            Err(e) => match e.kind() {
                // no such command.
                io::ErrorKind::NotFound => Ok(false),
                _ => Err(Error::from(e)),
            },
        }
    }

    /// NB: Only supported on Windows.
    #[cfg(not(windows))]
    pub fn test(&self) -> Result<bool, Error> {
        Ok(false)
    }

    /// List all the packages which are installed.
    pub fn install_packages<I>(&self, packages: I) -> Result<(), Error>
    where
        I: IntoIterator,
        I::Item: AsRef<str>,
    {
        for package in packages {
            let mut winget = self.winget.clone();
            winget.arg("install");
            winget.arg("-e");
            winget.arg(package.as_ref());
            winget.run()?;
        }

        Ok(())
    }

    /// List all the packages which are installed.
    #[cfg(windows)]
    pub fn list_installed(&self) -> Result<Vec<Package>, Error> {
        let mut out = Vec::new();

        for p in crate::ffi::win::msi::msi_enum_products()? {
            let mut it = p.name.split('.');

            match it.next_back().as_deref() {
                Some("msi") => (),
                _ => break,
            }

            let name = match (it.next(), it.next()) {
                (Some(a), Some(b)) if is_upper_camel(a) && is_upper_camel(b) => {
                    format!("{}.{}", a, b)
                }
                _ => continue,
            };

            match (it.next(), it.next(), it.next()) {
                (Some(a), Some(b), Some(c)) if is_num(a) && is_num(b) && is_num(c) => (),
                _ => continue,
            }

            if it.next().is_some() {
                continue;
            }

            out.push(Package { name })
        }

        return Ok(out);

        fn is_num(n: &str) -> bool {
            n.chars().all(char::is_numeric)
        }

        fn is_upper_camel(s: &str) -> bool {
            let mut it = s.chars();

            match it.next() {
                Some(a) if a.is_alphabetic() && a.is_uppercase() => (),
                _ => return false,
            }

            it.all(char::is_alphabetic)
        }
    }

    /// NB: Only supported on Windows.
    #[cfg(not(windows))]
    pub fn list_installed(&self) -> Result<Vec<Package>, Error> {
        let out = Vec::new();
        Ok(out)
    }
}

/// Packages abstraction for WinGet.
#[derive(Debug)]
pub struct PackageManager {
    winget: WinGet,
}

impl PackageManager {
    /// Construct a new winget package manager.
    pub fn new() -> Self {
        Self {
            winget: WinGet::new(),
        }
    }
}

impl super::PackageManager for PackageManager {
    fn primary(&self) -> bool {
        true
    }

    fn name(&self) -> &str {
        "winget"
    }

    /// Test that we have everything we need.
    fn test(&self) -> Result<bool, Error> {
        self.winget.test()
    }

    fn list_packages(&self) -> Result<Vec<Package>, Error> {
        self.winget.list_installed()
    }

    fn install_packages(&self, packages: &[String]) -> Result<(), Error> {
        self.winget.install_packages(packages)
    }
}
