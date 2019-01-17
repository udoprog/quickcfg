use crate::{command, os};
use failure::Error;
use std::io;
use std::path::{Path, PathBuf};

pub struct GitSystem {
    command: command::Command<'static>,
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
        match self.command.run(&["--version"]) {
            Ok(output) => return Ok(output.status.success()),
            Err(e) => match e.kind() {
                io::ErrorKind::NotFound => Ok(false),
                _ => return Err(e.into()),
            },
        }
    }

    fn clone(&self, url: &str, path: &Path) -> Result<Box<dyn super::Git>, Error> {
        use std::ffi::OsStr;

        let args = &[OsStr::new("clone"), OsStr::new(url), path.as_os_str()];
        self.command.run_checked(args)?;

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
    command: command::Command<'static>,
}

impl External {
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

impl super::Git for External {
    fn path(&self) -> &Path {
        &self.path
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
}
