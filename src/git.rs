//! Git abstraction.

use crate::{command, os};
use failure::Error;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

/// Helper to interact with a git repository.
#[derive(Debug)]
pub struct GitCommand {
    pub path: PathBuf,
    command: command::Command<'static>,
}

impl GitCommand {
    /// Construct a new git integration.
    pub fn new(path: impl AsRef<Path>) -> Result<GitCommand, Error> {
        Ok(GitCommand {
            path: path.as_ref().to_owned(),
            command: command::Command::new(os::command("git")),
        })
    }

    fn rev_parse(&self, git_ref: &str) -> Result<String, Error> {
        Ok(self
            .command
            .clone()
            .working_directory(self.path.as_path())
            .run_stdout(&["rev-parse", git_ref])?
            .trim()
            .to_string())
    }

    /// Find the merge base between two commits.
    fn merge_base(&self, a: &str, b: &str) -> Result<String, Error> {
        Ok(self
            .command
            .clone()
            .working_directory(self.path.as_path())
            .run_stdout(&["merge-base", a, b])?
            .trim()
            .to_string())
    }
}

impl Git for GitCommand {
    fn path(&self) -> &Path {
        &self.path
    }

    fn test(&self) -> Result<bool, Error> {
        match self.command.run(&["--version"]) {
            Ok(output) => return Ok(output.status.success()),
            Err(e) => match e.kind() {
                io::ErrorKind::NotFound => Ok(false),
                _ => return Err(e.into()),
            },
        }
    }

    fn needs_update(&self) -> Result<bool, Error> {
        self.command
            .clone()
            .working_directory(self.path())
            .run_checked(&["fetch", "origin", "master"])?;

        let remote_head = self.rev_parse("FETCH_HEAD")?;
        let head = self.rev_parse("HEAD")?;

        if remote_head != head {
            // check if remote is a base
            return Ok(self.merge_base(&remote_head, &head)? != remote_head);
        }

        Ok(false)
    }

    fn is_fresh(&self) -> Result<bool, Error> {
        let output = self
            .command
            .clone()
            .working_directory(self.path.as_path())
            .run(&["diff-index", "--quiet", "HEAD"])?;

        Ok(output.status.success())
    }

    fn force_update(&self) -> Result<(), Error> {
        self.command
            .clone()
            .working_directory(self.path.as_path())
            .run_checked(&["reset", "--hard", "FETCH_HEAD"])
    }

    fn update(&self) -> Result<(), Error> {
        self.command
            .clone()
            .working_directory(self.path.as_path())
            .run_checked(&["merge", "--ff-only", "FETCH_HEAD"])
    }

    fn clone_remote(&self, remote: &str) -> Result<(), Error> {
        use std::ffi::OsStr;

        let args = &[
            OsStr::new("clone"),
            OsStr::new(remote),
            self.path.as_os_str(),
        ];

        self.command.run_checked(args)
    }
}

pub trait Git: Send + Sync + fmt::Debug {
    /// The path this git instance is associated with.
    fn path(&self) -> &Path;

    /// Test if git command works.
    fn test(&self) -> Result<bool, Error>;

    /// Check if repo needs to be updated.
    fn needs_update(&self) -> Result<bool, Error>;

    /// Check if the local repository has not been modified without comitting.
    fn is_fresh(&self) -> Result<bool, Error>;

    /// Force update repo.
    fn force_update(&self) -> Result<(), Error>;

    /// Update repo.
    fn update(&self) -> Result<(), Error>;

    /// Clone a remote to the current repo.
    fn clone_remote(&self, remote: &str) -> Result<(), Error>;
}

/// Open the given path.
pub fn open(path: impl AsRef<Path>) -> Result<Box<Git>, Error> {
    Ok(Box::new(GitCommand::new(path)?))
}
