//! Unix-specific implementations.

use crate::unit::{AddMode, Symlink};
use anyhow::{Context, Result, anyhow};
use std::borrow::Cow;
use std::fs::Metadata;
use std::path::{Path, PathBuf};

/// Convert into an executable path.
pub fn exe_path(path: &Path) -> Cow<'_, Path> {
    Cow::Borrowed(path)
}

/// Convert the given command into a path.
///
/// This adds the platform-specific extension for Windows.
pub fn command(base: &str) -> Cow<'_, Path> {
    Cow::from(Path::new(base))
}

/// Detect git command.
#[allow(unused)]
pub fn detect_git() -> Result<PathBuf> {
    Ok(PathBuf::from("git"))
}

/// Add the given modes (on top of the existing ones).
pub fn add_mode(add_mode: &AddMode) -> Result<()> {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    let mut perm = add_mode.path.metadata()?.permissions();
    let mode = perm.mode() | add_mode.unix_mode();
    perm.set_mode(mode);

    fs::set_permissions(&add_mode.path, perm)
        .with_context(|| anyhow!("failed to add mode: {}", add_mode.path.display()))?;

    Ok(())
}

/// Test if the permission bits of `from` and `to` differ.
pub fn permissions_differ(from: &Metadata, to: &Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;

    /// Mask off the file type bits, leaving permissions and setuid/gid/sticky.
    const MASK: u32 = 0o7777;

    from.permissions().mode() & MASK != to.permissions().mode() & MASK
}

/// Copy the permissions of `from` onto `to`.
pub fn copy_permissions(from: &Path, to: &Path) -> Result<()> {
    use std::fs;

    let perm = from
        .metadata()
        .with_context(|| anyhow!("failed to read metadata: {}", from.display()))?
        .permissions();

    fs::set_permissions(to, perm)
        .with_context(|| anyhow!("failed to copy permissions: {}", to.display()))?;

    Ok(())
}

/// Create a symlink.
pub fn create_symlink(symlink: &Symlink) -> Result<()> {
    use std::{fs, os::unix};

    let Symlink {
        remove,
        ref path,
        ref link,
    } = *symlink;

    if remove {
        log::info!("re-linking {} to {}", path.display(), link.display());
        fs::remove_file(path)?;
    } else {
        log::info!("linking {} to {}", path.display(), link.display());
    }

    unix::fs::symlink(link, path)?;
    Ok(())
}
