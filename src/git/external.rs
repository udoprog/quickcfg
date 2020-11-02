use crate::{command, os};
use anyhow::Error;
use std::io;
use std::path::{Path, PathBuf};

pub struct GitSystem {
    command: command::Command,
}

impl GitSystem {
    pub fn new() -> Self {
        Self {
            command: command::Command::new(os::command("git")),
        }
    }
}

impl super::GitSystem for GitSystem {
    fn test(&self) -> Result<bool, Error> {
        let mut command = self.command.clone();
        command.arg("--version");

        match command.run() {
            Ok(output) => Ok(output.status.success()),
            Err(e) => match e.kind() {
                io::ErrorKind::NotFound => Ok(false),
                _ => Err(e.into()),
            },
        }
    }

    fn clone(&self, url: &str, path: &Path) -> Result<Box<dyn super::Git>, Error> {
        let mut command = self.command.clone();
        command.arg("clone");
        command.arg(url);
        command.arg(path);
        command.run_checked()?;

        Ok(Box::new(External {
            path: path.to_owned(),
            command: self.command.clone(),
        }))
    }

    fn open(&self, path: &Path) -> Result<Box<dyn super::Git>, Error> {
        Ok(Box::new(External {
            path: path.to_owned(),
            command: self.command.clone(),
        }))
    }
}

/// Helper to interact with a git repository through an external command.
#[derive(Debug)]
struct External {
    pub path: PathBuf,
    command: command::Command,
}

impl External {
    fn rev_parse(&self, git_ref: &str) -> Result<String, Error> {
        let mut command = self.command.clone();
        command.working_directory(&self.path);
        command.args(&["rev-parse", git_ref]);
        Ok(command.run_stdout()?.trim().to_string())
    }

    /// Find the merge base between two commits.
    fn merge_base(&self, a: &str, b: &str) -> Result<String, Error> {
        let mut command = self.command.clone();
        command.working_directory(&self.path);
        command.args(&["merge-base", a, b]);
        Ok(command.run_stdout()?.trim().to_string())
    }
}

impl super::Git for External {
    fn path(&self) -> &Path {
        &self.path
    }

    fn needs_update(&self) -> Result<bool, Error> {
        let head = self.rev_parse("HEAD")?;

        let mut command = self.command.clone();
        command.working_directory(self.path());
        command.args(&["fetch", "origin", head.as_str()]);
        command.run_checked()?;

        let remote_head = self.rev_parse("FETCH_HEAD")?;

        if remote_head != head {
            // check if remote is a base
            return Ok(self.merge_base(&remote_head, &head)? != remote_head);
        }

        Ok(false)
    }

    fn is_fresh(&self) -> Result<bool, Error> {
        let mut command = self.command.clone();
        command.working_directory(&self.path);
        command.args(&["diff-index", "--quiet", "HEAD"]);
        Ok(command.status()?.success())
    }

    fn force_update(&self) -> Result<(), Error> {
        let mut command = self.command.clone();
        command.working_directory(&self.path);
        command.args(&["reset", "--hard", "FETCH_HEAD"]);
        command.run_checked()
    }

    fn update(&self) -> Result<(), Error> {
        let mut command = self.command.clone();
        command.working_directory(&self.path);
        command.args(&["merge", "--ff-only", "FETCH_HEAD"]);
        command.run_checked()
    }
}
