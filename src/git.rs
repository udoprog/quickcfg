//! Git abstraction.

use crate::command;
use failure::Error;
use std::path::{Path, PathBuf};

/// Helper to interact with a git repository.
pub struct Git {
    command: command::Command<'static>,
}

impl Git {
    /// Construct a new git integration.
    pub fn new<'a>(path: impl Into<&'a Path>) -> Git {
        Git {
            command: command::Command::new("git").working_directory(path),
        }
    }

    /// Get the HEAD of the current repository as a commit id.
    pub fn get_head(&self) -> Result<String, Error> {
        Ok(self
            .command
            .run_out(&["rev-parse", "HEAD"])?
            .trim()
            .to_string())
    }

    /// Find the merge base between two commits.
    pub fn merge_base(&self, a: &str, b: &str) -> Result<String, Error> {
        Ok(self
            .command
            .run_out(&["merge-base", a, b])?
            .trim()
            .to_string())
    }

    /// Check if repo needs to be updated.
    pub fn needs_update(&self) -> Result<bool, Error> {
        self.command.run(&["fetch", "origin", "master"])?;

        let remote_head = self.command.run_out(&["rev-parse", "FETCH_HEAD"])?;
        let remote_head = remote_head.trim();
        let head = self.get_head()?;

        if remote_head != head {
            // check if remote is a base
            return Ok(self.merge_base(remote_head, &head)? != remote_head);
        }

        Ok(false)
    }

    /// Check if the local repository has not been modified without comitting.
    pub fn is_fresh(&self) -> Result<bool, Error> {
        Ok(self
            .command
            .run_status(&["diff-index", "--quiet", "HEAD"])?
            .success())
    }

    /// Force update repo.
    pub fn force_update(&self) -> Result<(), Error> {
        self.command.run(&["reset", "--hard", "FETCH_HEAD"])
    }

    /// Update repo.
    pub fn update(&self) -> Result<(), Error> {
        self.command.run(&["merge", "--ff-only", "FETCH_HEAD"])
    }
}
