//! Helper to run external commands.

use anyhow::{bail, Error};
use std::borrow::Cow;
use std::ffi::OsStr;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::process;
use thiserror::Error;

/// The decoded output after running a command.
pub struct Output {
    pub status: process::ExitStatus,
    pub stdout: String,
    pub stderr: String,
}

impl Output {
    /// Convert output into a formatted error.
    pub fn into_error(self) -> OutputError {
        OutputError {
            status: self.status,
            stdout: self.stdout,
            stderr: self.stderr,
        }
    }
}

#[derive(Debug, Error)]
pub struct OutputError {
    pub status: process::ExitStatus,
    pub stdout: String,
    pub stderr: String,
}

impl fmt::Display for OutputError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        writeln!(fmt, "process exited with status: {}", self.status)?;

        if !self.stdout.is_empty() {
            writeln!(fmt, "stdout:")?;
            self.stdout.fmt(fmt)?;
        }

        if !self.stderr.is_empty() {
            writeln!(fmt, "stderr:")?;
            self.stderr.fmt(fmt)?;
        }

        Ok(())
    }
}

/// A command wrapper that simplifies interaction with external commands.
#[derive(Debug, Clone)]
pub struct Command<'name> {
    name: Cow<'name, Path>,
    working_directory: Option<PathBuf>,
}

impl<'name> Command<'name> {
    /// Construct a new command wrapper.
    pub fn new(name: Cow<'name, Path>) -> Command<'name> {
        Command {
            name,
            working_directory: None,
        }
    }

    fn command<S>(&self, args: impl IntoIterator<Item = S>) -> process::Command
    where
        S: AsRef<OsStr>,
    {
        let mut cmd = process::Command::new(self.name.as_os_str());
        cmd.args(args);

        if let Some(working_directory) = self.working_directory.as_ref() {
            cmd.current_dir(working_directory);
        }

        cmd
    }

    /// Configure the working directory of this command.
    pub fn working_directory<'p>(self, path: impl Into<&'p Path>) -> Self {
        Command {
            name: self.name,
            working_directory: Some(path.into().to_owned()),
        }
    }

    /// Run the given command, return all lines printed to stdout on success.
    pub fn run_lines<S>(&self, args: impl IntoIterator<Item = S>) -> Result<Vec<String>, Error>
    where
        S: AsRef<OsStr>,
    {
        let lines = self
            .run_stdout(args)?
            .split('\n')
            .map(|s| s.to_string())
            .collect();

        Ok(lines)
    }

    /// Run the given command, return a string of all output.
    pub fn run_stdout<S>(&self, args: impl IntoIterator<Item = S>) -> Result<String, Error>
    where
        S: AsRef<OsStr>,
    {
        let output = self.run(args)?;

        if !output.status.success() {
            return Err(Error::from(output.into_error()));
        }

        Ok(output.stdout)
    }

    /// Run the given command, only checking for status code and providing diagnostics.
    pub fn run_checked<S>(&self, args: impl IntoIterator<Item = S>) -> Result<(), Error>
    where
        S: AsRef<OsStr>,
    {
        let output = self.run(args)?;

        if !output.status.success() {
            return Err(Error::from(output.into_error()));
        }

        Ok(())
    }

    /// Run the given command, inheriting stdout, stderr from the current process.
    ///
    /// This is discouraged, since it basically requires the command to be running on the main
    /// thread.
    pub fn run_inherited<S>(&self, args: impl IntoIterator<Item = S>) -> Result<(), Error>
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

    /// Run the given command, return a string of all output.
    pub fn run<S>(&self, args: impl IntoIterator<Item = S>) -> Result<Output, io::Error>
    where
        S: AsRef<OsStr>,
    {
        let output = self.command(args).output()?;

        let output = Output {
            status: output.status,
            stdout: String::from_utf8(output.stdout).map_err(|_| {
                io::Error::new(io::ErrorKind::Other, "Cannot decode stdout as utf-8")
            })?,
            stderr: String::from_utf8(output.stderr).map_err(|_| {
                io::Error::new(io::ErrorKind::Other, "Cannot decode stderr as utf-8")
            })?,
        };

        Ok(output)
    }
}
