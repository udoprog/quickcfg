//! Git abstraction.

use crate::command;
use failure::{bail, Error};
use std::path::{Path, PathBuf};

/// Helper to interact with a git repository.
#[derive(Debug)]
pub struct Git {
    pub path: PathBuf,
    command: command::Command<'static>,
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
        Ok(self.command.run_status(&["--version"])?.success())
    }

    /// Check if repo needs to be updated.
    pub fn needs_update(&self) -> Result<bool, Error> {
        self.command
            .clone()
            .working_directory(self.path.as_path())
            .run(&["fetch", "origin", "master"])?;

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
        Ok(self
            .command
            .clone()
            .working_directory(self.path.as_path())
            .run_status(&["diff-index", "--quiet", "HEAD"])?
            .success())
    }

    /// Force update repo.
    pub fn force_update(&self) -> Result<(), Error> {
        self.command
            .clone()
            .working_directory(self.path.as_path())
            .run(&["reset", "--hard", "FETCH_HEAD"])
    }

    /// Update repo.
    pub fn update(&self) -> Result<(), Error> {
        self.command
            .clone()
            .working_directory(self.path.as_path())
            .run(&["merge", "--ff-only", "FETCH_HEAD"])
    }

    /// Clone a remote to the current repo.
    pub fn clone(&self, remote: &str) -> Result<(), Error> {
        use std::ffi::OsStr;

        let args = &[
            OsStr::new("clone"),
            OsStr::new(remote),
            self.path.as_os_str(),
        ];

        if !self.command.run_status(args)?.success() {
            bail!("Failed to clone");
        }

        Ok(())
    }

    /// Get the HEAD of the current repository as a commit id.
    fn rev_parse(&self, git_ref: &str) -> Result<String, Error> {
        Ok(self
            .command
            .clone()
            .working_directory(self.path.as_path())
            .run_out(&["rev-parse", git_ref])?
            .trim()
            .to_string())
    }

    /// Find the merge base between two commits.
    fn merge_base(&self, a: &str, b: &str) -> Result<String, Error> {
        Ok(self
            .command
            .clone()
            .working_directory(self.path.as_path())
            .run_out(&["merge-base", a, b])?
            .trim()
            .to_string())
    }
}
