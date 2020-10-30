//! Unix-specific implementations.

use crate::unit::{AddMode, Symlink};
use anyhow::{anyhow, Context as _, Error};
use std::borrow::Cow;
use std::path::{Path, PathBuf};

/// Convert into an executable path.
pub fn exe_path(path: PathBuf) -> PathBuf {
    path
}

/// Convert the given command into a path.
///
/// This adds the platform-specific extension for Windows.
pub fn command(base: &str) -> Cow<'_, Path> {
    Cow::from(Path::new(base))
}

/// Detect git command.
#[allow(unused)]
pub fn detect_git() -> Result<PathBuf, Error> {
    Ok(PathBuf::from("git"))
}

/// Add the given modes (on top of the existing ones).
pub fn add_mode(add_mode: &AddMode) -> Result<(), Error> {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    let mut perm = add_mode.path.metadata()?.permissions();
    let mode = perm.mode() | add_mode.unix_mode();
    perm.set_mode(mode);

    fs::set_permissions(&add_mode.path, perm)
        .with_context(|| anyhow!("failed to add mode: {}", add_mode.path.display()))?;

    Ok(())
}

/// Create a symlink.
pub fn create_symlink(symlink: &Symlink) -> Result<(), Error> {
    use std::{fs, os::unix};

    let Symlink {
        remove,
        ref path,
        ref link,
    } = *symlink;

    if remove {
        log::info!("re-linking {} to {}", path.display(), link.display());
        fs::remove_file(&path)?;
    } else {
        log::info!("linking {} to {}", path.display(), link.display());
    }

    unix::fs::symlink(link, path)?;
    Ok(())
}
