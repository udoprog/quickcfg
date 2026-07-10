//! Windows-specific implementations.

use crate::unit::{AddMode, Symlink};
use anyhow::{Error, bail};
use std::borrow::Cow;
use std::env::consts;
use std::fs::Metadata;
use std::path::Path;

/// Convert into an executable path.
pub fn exe_path<P>(path: &P) -> Cow<'_, Path>
where
    P: ?Sized + AsRef<Path>,
{
    let path = path.as_ref();

    if path.extension() == Some(consts::EXE_EXTENSION.as_ref()) {
        return Cow::Borrowed(path);
    }

    let mut path = path.to_owned();
    path.set_extension(consts::EXE_EXTENSION);
    Cow::Owned(path)
}

/// Convert the given command into a path.
///
/// This adds the platform-specific extension for Windows.
pub fn command<P>(base: &P) -> Cow<'_, Path>
where
    P: ?Sized + AsRef<Path>,
{
    exe_path(base)
}

/// Add the given modes (on top of the existing ones).
pub fn add_mode(mode: &AddMode) -> Result<(), Error> {
    if mode.is_executable() {
        // NB: windows files are executable if they have the .exe extension.
        if mode.path.extension() != Some(consts::EXE_EXTENSION.as_ref()) {
            bail!("non-exe files cannot be executable");
        }
    }

    Ok(())
}

/// Test if the permission bits of `from` and `to` differ.
///
/// NB: see [copy_permissions], there is nothing for us to sync on windows.
pub fn permissions_differ(_: &Metadata, _: &Metadata) -> bool {
    false
}

/// Copy the permissions of `from` onto `to`.
///
/// NB: the only permission windows exposes through `fs::Permissions` is the
/// readonly flag. Propagating it would mark the destination readonly and cause
/// the next copy over it to fail, so this is deliberately a no-op.
pub fn copy_permissions(_: &Path, _: &Path) -> Result<(), Error> {
    Ok(())
}

/// Create a symlink.
pub fn create_symlink(symlink: &Symlink) -> Result<(), Error> {
    use std::fs;
    use std::os::windows::fs::{symlink_dir, symlink_file};

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

    if path.is_file() {
        symlink_file(path, path.join(link))?;
        return Ok(());
    }

    if path.is_dir() {
        symlink_dir(path, path.join(link))?;
        return Ok(());
    }

    bail!(
        "cannot symlink `{}`: not a file or directory",
        path.display()
    );
}
