//! Helper to run external commands.

use anyhow::{bail, Error};
use std::ffi::{OsStr, OsString};
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
pub struct Command {
    pub(crate) name: PathBuf,
    pub(crate) working_directory: Option<PathBuf>,
    pub(crate) args: Vec<OsString>,
}

impl Command {
    /// Construct a new command wrapper.
    pub fn new(name: impl Into<PathBuf>) -> Command {
        Command {
            name: name.into(),
            working_directory: None,
            args: Vec::new(),
        }
    }

    /// Push an argument to the command.
    pub fn arg<A>(&mut self, arg: A)
    where
        A: AsRef<OsStr>,
    {
        self.args.push(arg.as_ref().to_owned());
    }

    /// Push a collection of arguments to the command.
    pub fn args<I>(&mut self, args: I)
    where
        I: IntoIterator,
        I::Item: AsRef<OsStr>,
    {
        self.args
            .extend(args.into_iter().map(|arg| arg.as_ref().to_owned()));
    }

    fn command(&self) -> process::Command {
        let mut cmd = process::Command::new(self.name.as_os_str());
        cmd.args(&self.args);

        if let Some(working_directory) = self.working_directory.as_ref() {
            cmd.current_dir(working_directory);
        }

        cmd
    }

    /// Configure the working directory of this command.
    pub fn working_directory(&mut self, path: impl AsRef<Path>) {
        self.working_directory = Some(path.as_ref().to_owned());
    }

    /// Run the given command, return all lines printed to stdout on success.
    pub fn run_lines(self) -> Result<Vec<String>, Error> {
        let lines = self
            .run_stdout()?
            .split('\n')
            .map(|s| s.to_string())
            .collect();

        Ok(lines)
    }

    /// Run the given command, return a string of all output.
    pub fn run_stdout(self) -> Result<String, Error> {
        let output = self.run()?;

        if !output.status.success() {
            return Err(Error::from(output.into_error()));
        }

        Ok(output.stdout)
    }

    /// Run the given command, only checking for status code and providing diagnostics.
    pub fn run_checked(self) -> Result<(), Error> {
        let output = self.run()?;

        if !output.status.success() {
            return Err(Error::from(output.into_error()));
        }

        Ok(())
    }

    /// Run the given command, inheriting stdout, stderr from the current process.
    ///
    /// This is discouraged, since it basically requires the command to be running on the main
    /// thread.
    pub fn run_inherited(&self) -> Result<(), Error> {
        let mut cmd = self.command();
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
    pub fn run(self) -> io::Result<Output> {
        let output = self.command().output()?;

        let output = Output {
            status: output.status,
            stdout: String::from_utf8(output.stdout)
                .map_err(|_| io::Error::other("Cannot decode stdout as utf-8"))?,
            stderr: String::from_utf8(output.stderr)
                .map_err(|_| io::Error::other("Cannot decode stderr as utf-8"))?,
        };

        Ok(output)
    }

    /// Run the command and wait for exit status.
    pub fn status(self) -> io::Result<process::ExitStatus> {
        self.command().status()
    }

    /// Run as administrator.
    #[cfg(windows)]
    pub fn runas(self) -> io::Result<i32> {
        crate::ffi::win::shellapi::runas(self)
    }
}
