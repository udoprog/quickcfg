//! Git abstraction.

use failure::Error;
use std::fmt;
use std::path::Path;

#[cfg(not(feature = "git2"))]
#[path = "git/external.rs"]
mod system;
#[cfg(feature = "git2")]
#[path = "git/git2.rs"]
mod system;

pub trait Git: Send + fmt::Debug {
    /// The path this git instance is associated with.
    fn path(&self) -> &Path;

    /// Check if repo needs to be updated.
    fn needs_update(&self) -> Result<bool, Error>;

    /// Check if the local repository has not been modified without comitting.
    fn is_fresh(&self) -> Result<bool, Error>;

    /// Force update repo.
    fn force_update(&self) -> Result<(), Error>;

    /// Update repo.
    fn update(&self) -> Result<(), Error>;
}

pub trait GitSystem: Send + Sync {
    fn test(&self) -> Result<bool, Error> {
        Ok(true)
    }

    /// Clone the given path.
    fn clone(&self, url: &str, path: &Path) -> Result<Box<dyn Git>, Error>;

    /// Open the given repository.
    fn open(&self, path: &Path) -> Result<Box<dyn Git>, Error>;
}

/// Open the given path.
pub fn setup() -> Result<Box<dyn GitSystem>, Error> {
    Ok(Box::new(system::GitSystem::new()))
}
