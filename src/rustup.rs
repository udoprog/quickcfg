//! Packages abstraction for rustup.

use crate::{command, os, packages::Package};
use anyhow::{anyhow, Error};
use std::ffi::OsStr;
use std::io;

#[derive(Debug)]
pub struct Rustup {
    rustup: command::Command<'static>,
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
        match self.rustup.run(&["--version"]) {
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
        args.push(OsStr::new(self.sub_command));
        args.push(OsStr::new(self.install));
        args.extend(packages.iter().map(AsRef::as_ref));

        self.rustup.run(args)?;
        Ok(())
    }

    /// List all the packages which are installed.
    pub fn list_installed(&self) -> Result<Vec<Package>, Error> {
        use std::env::consts;

        let mut out = Vec::new();

        for line in self.rustup.run_lines(&[self.sub_command, "list"])? {
            if line.starts_with(char::is_whitespace) {
                continue;
            }

            let line = line.trim();

            if line == "" {
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
