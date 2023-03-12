//! Packages abstraction for rustup.

use crate::{command, os, packages::Package};
use anyhow::{anyhow, Error};
use std::ffi::OsStr;
use std::io;

#[derive(Debug)]
pub struct Rustup {
    rustup: command::Command,
    sub_command: &'static str,
    install: &'static str,
}

impl Rustup {
    /// Create a new rustup command wrapper.
    pub fn new(sub_command: &'static str, install: &'static str) -> Self {
        Rustup {
            rustup: command::Command::new(os::command("rustup")),
            sub_command,
            install,
        }
    }

    /// Test that the command is available.
    pub fn test(&self) -> Result<bool, Error> {
        let mut rustup = self.rustup.clone();
        rustup.arg("--version");

        match rustup.run() {
            Ok(output) => Ok(output.status.success()),
            Err(e) => match e.kind() {
                // no such command.
                io::ErrorKind::NotFound => Ok(false),
                _ => Err(Error::from(e)),
            },
        }
    }

    /// List all the packages which are installed.
    pub fn install_packages<I>(&self, packages: I) -> Result<(), Error>
    where
        I: IntoIterator,
        I::Item: AsRef<OsStr>,
    {
        let packages = packages.into_iter().collect::<Vec<_>>();

        let mut rustup = self.rustup.clone();
        rustup.arg(self.sub_command);
        rustup.arg(self.install);
        rustup.args(packages);
        rustup.run()?;
        Ok(())
    }

    /// List all the packages which are installed.
    pub fn list_installed(&self) -> Result<Vec<Package>, Error> {
        use std::env::consts;

        let mut out = Vec::new();

        let mut rustup = self.rustup.clone();
        rustup.arg(self.sub_command);
        rustup.arg("list");

        for line in rustup.run_lines()? {
            if line.starts_with(char::is_whitespace) {
                continue;
            }

            let line = line.trim();

            if line.is_empty() {
                continue;
            }

            let mut it = line.split(' ');

            let name = it.next().ok_or_else(|| anyhow!("expected package name"))?;

            match it.next() {
                Some("(default)") => {}
                Some("(installed)") => {}
                _ => continue,
            }

            let name = match name.find(consts::ARCH) {
                Some(index) => name[..index].trim_matches('-'),
                None => continue,
            };

            out.push(Package {
                name: name.to_string(),
            });
        }

        Ok(out)
    }
}
