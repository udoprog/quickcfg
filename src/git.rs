//! Git abstraction.

use crate::command;
use std::path::{Path, PathBuf};
use std::io;
use failure::Error;

/// Helper to interact with a git repository.
#[derive(Debug)]
pub struct Git {
    pub path: PathBuf,
    command: command::Command,
}

impl Git {
    /// Construct a new git integration.
    pub fn new<'a>(path: impl Into<&'a Path>) -> Git {
        Git {
            path: path.into().to_owned(),
            command: command::Command::new("git"),
        }
    }

    /// Test if git command works.
    pub fn test(&self) -> Result<bool, Error> {
        match self.command.run(&["--version"]) {
            Ok(output) => return Ok(output.status.success()),
            Err(e) => match e.kind() {
                io::ErrorKind::NotFound => Ok(false),
                _ => return Err(e.into()),
            }
        }
    }

    /// Check if repo needs to be updated.
    pub fn needs_update(&self) -> Result<bool, Error> {
        self.command
            .clone()
            .working_directory(self.path.as_path())
            .run_checked(&["fetch", "origin", "master"])?;

        let remote_head = self.rev_parse("FETCH_HEAD")?;
        let head = self.rev_parse("HEAD")?;

        if remote_head != head {
            // check if remote is a base
            return Ok(self.merge_base(&remote_head, &head)? != remote_head);
        }

        Ok(false)
    }

    /// Check if the local repository has not been modified without comitting.
    pub fn is_fresh(&self) -> Result<bool, Error> {
        let output = self
            .command
            .clone()
            .working_directory(self.path.as_path())
            .run(&["diff-index", "--quiet", "HEAD"])?;

        Ok(output.status.success())
    }

    /// Force update repo.
    pub fn force_update(&self) -> Result<(), Error> {
        self.command
            .clone()
            .working_directory(self.path.as_path())
            .run_checked(&["reset", "--hard", "FETCH_HEAD"])
    }

    /// Update repo.
    pub fn update(&self) -> Result<(), Error> {
        self.command
            .clone()
            .working_directory(self.path.as_path())
            .run_checked(&["merge", "--ff-only", "FETCH_HEAD"])
    }

    /// Clone a remote to the current repo.
    pub fn clone(&self, remote: &str) -> Result<(), Error> {
        use std::ffi::OsStr;

        let args = &[
            OsStr::new("clone"),
            OsStr::new(remote),
            self.path.as_os_str(),
        ];

        self.command.run_checked(args)
    }

    /// Get the HEAD of the current repository as a commit id.
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
