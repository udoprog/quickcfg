//! Helper to run external commands.

use failure::{bail, Error};
use std::borrow::Cow;
use std::ffi::OsStr;
use std::process;

/// A command wrapper that simplifies interaction with external commands.
#[derive(Debug)]
pub struct Command<'a> {
    name: Cow<'a, str>,
}

impl<'a> Command<'a> {
    /// Construct a new command wrapper.
    pub fn new(name: impl Into<Cow<'a, str>>) -> Command<'a> {
        Command { name: name.into() }
    }

    /// Run the given command, inheriting stdout, stderr from the current process.
    pub fn run<S>(&self, args: impl IntoIterator<Item = S>) -> Result<(), Error>
    where
        S: AsRef<OsStr>,
    {
        let mut cmd = process::Command::new(self.name.as_ref());
        cmd.args(args);
        let status = cmd.status()?;

        if !status.success() {
            bail!(
                "Command exited with non-zero status: {:?}: {:?}",
                cmd,
                status
            );
        }

        Ok(())
    }

    /// Run the given command, return all lines printed to stdout on success.
    pub fn run_lines<S>(&self, args: impl IntoIterator<Item = S>) -> Result<Vec<String>, Error>
    where
        S: AsRef<OsStr>,
    {
        let mut cmd = process::Command::new(self.name.as_ref());
        cmd.args(args);
        let output = cmd.output()?;

        if !output.status.success() {
            bail!(
                "Command exited with non-zero status: {:?}: {:?}",
                cmd,
                output.status
            );
        }

        let lines = std::str::from_utf8(&output.stdout)?
            .split("\n")
            .map(|s| s.to_string())
            .collect();
        Ok(lines)
    }
}
