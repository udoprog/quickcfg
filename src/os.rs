#[cfg(windows)]
#[path = "os/windows.rs"]
mod internal;

#[cfg(unix)]
#[path = "os/unix.rs"]
mod internal;

pub use self::internal::*;
