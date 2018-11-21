//! Helper to run external commands.

use failure::{bail, Error};
use std::borrow::Cow;
use std::ffi::OsStr;
use std::process;
use std::path::{Path, PathBuf};

/// A command wrapper that simplifies interaction with external commands.
#[derive(Debug, Clone)]
pub struct Command<'a> {
    name: Cow<'a, str>,
    working_directory: Option<PathBuf>,
}

impl<'a> Command<'a> {
    /// Construct a new command wrapper.
    pub fn new(name: impl Into<Cow<'a, str>>) -> Command<'a> {
        Command { name: name.into(), working_directory: None }
    }

    /// Configure the working directory of this command.
    pub fn working_directory<'p>(self, path: impl Into<&'p Path>) -> Self {
        Command {
            name: self.name,
            working_directory: Some(path.into().to_owned()),
        }
    }

    fn command<S>(&self, args: impl IntoIterator<Item = S>) -> process::Command
    where
        S: AsRef<OsStr>,
    {
        let mut cmd = process::Command::new(self.name.as_ref());
        cmd.args(args);

        if let Some(working_directory) = self.working_directory.as_ref() {
            cmd.current_dir(working_directory);
        }

        cmd
    }

    /// Run the given command, inheriting stdout, stderr from the current process.
    pub fn run<S>(&self, args: impl IntoIterator<Item = S>) -> Result<(), Error>
    where
        S: AsRef<OsStr>,
    {
        let mut cmd = self.command(args);
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
        let mut cmd = self.command(args);
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

    /// Run the given command, return a string of all output.
    pub fn run_out<S>(&self, args: impl IntoIterator<Item = S>) -> Result<String, Error>
    where
        S: AsRef<OsStr>,
    {
        let mut cmd = self.command(args);
        let output = cmd.output()?;

        if !output.status.success() {
            bail!(
                "Command exited with non-zero status: {:?}: {:?}",
                cmd,
                output.status
            );
        }

        Ok(std::str::from_utf8(&output.stdout)?.to_string())
    }

    /// Run the given command, return a string of all output.
    pub fn run_status<S>(
        &self,
        args: impl IntoIterator<Item = S>,
    ) -> Result<process::ExitStatus, Error>
    where
        S: AsRef<OsStr>,
    {
        Ok(self.command(args).status()?)
    }
}
