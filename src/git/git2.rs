//! Git integration using libgit2

use anyhow::{bail, format_err, Error};
use git2::{ObjectType, Oid, Repository, ResetType};
use std::fmt;
use std::path::{Path, PathBuf};

pub struct GitSystem(());

impl GitSystem {
    pub fn new() -> Self {
        GitSystem(())
    }
}

impl super::GitSystem for GitSystem {
    fn clone(&self, url: &str, path: &Path) -> Result<Box<dyn super::Git>, Error> {
        Ok(Box::new(Git2 {
            path: path.to_owned(),
            repo: Repository::clone(url, path)?,
        }))
    }

    fn open(&self, path: &Path) -> Result<Box<dyn super::Git>, Error> {
        Ok(Box::new(Git2 {
            path: path.to_owned(),
            repo: Repository::open(path)?,
        }))
    }
}

/// Helper to interact with a git repository.
pub struct Git2 {
    pub path: PathBuf,
    pub repo: Repository,
}

impl fmt::Debug for Git2 {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("Git2").field("path", &self.path).finish()
    }
}

impl Git2 {
    fn rev_parse(&self, git_ref: &str) -> Result<Oid, Error> {
        let spec = self.repo.revparse(git_ref)?;

        if !spec.mode().contains(git2::RevparseMode::SINGLE) {
            bail!("bad rev spec");
        }

        let from = spec.from().ok_or_else(|| format_err!("missing `from`"))?;
        Ok(from.id())
    }

    /// Find the merge base between two commits.
    fn merge_base(&self, a: Oid, b: Oid) -> Result<Oid, Error> {
        Ok(self.repo.merge_base(a, b)?)
    }
}

impl super::Git for Git2 {
    fn path(&self) -> &Path {
        &self.path
    }

    fn needs_update(&self) -> Result<bool, Error> {
        let mut remote = self.repo.find_remote("origin")?;
        remote.fetch(&["master"], None, None)?;

        let fetch_head = self.rev_parse("FETCH_HEAD")?;
        let head = self.rev_parse("HEAD")?;

        if fetch_head != head {
            // check if remote is a base
            return Ok(self.merge_base(fetch_head, head)? != fetch_head);
        }

        Ok(false)
    }

    fn is_fresh(&self) -> Result<bool, Error> {
        let diff = self.repo.diff_index_to_workdir(None, None)?;
        Ok(diff.deltas().next().is_none())
    }

    fn force_update(&self) -> Result<(), Error> {
        let fetch_head = self.rev_parse("FETCH_HEAD")?;
        let fetch_head = self
            .repo
            .find_object(fetch_head, Some(ObjectType::Commit))?;
        self.repo.reset(&fetch_head, ResetType::Hard, None)?;
        Ok(())
    }

    fn update(&self) -> Result<(), Error> {
        // NB: needs --ff-only
        let fetch_head = self
            .repo
            .find_annotated_commit(self.rev_parse("FETCH_HEAD")?)?;
        self.repo.merge(&[&fetch_head], None, None)?;
        Ok(())
    }
}
