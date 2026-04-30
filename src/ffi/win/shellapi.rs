use core::mem;

use std::ffi::{OsStr, OsString};
use std::io;
use std::os::windows::ffi::OsStrExt as _;
use std::ptr;

use windows_sys::Win32::Foundation::{TRUE, WAIT_ABANDONED, WAIT_OBJECT_0, WAIT_TIMEOUT};
use windows_sys::Win32::System::Threading::{GetExitCodeProcess, INFINITE, WaitForSingleObject};
use windows_sys::Win32::UI::Shell::{SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW};
use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOW;

fn wide_string(s: impl AsRef<OsStr>) -> Vec<u16> {
    s.as_ref().encode_wide().chain(Some(0)).collect::<Vec<_>>()
}

pub fn runas(command: crate::Command) -> io::Result<i32> {
    let operation: Vec<u16> = OsStr::new("runas\0").encode_wide().collect();

    let file = wide_string(&command.name);
    let params = wide_string(encode_params(&command.args)?);

    let mut exit_code = 0;

    unsafe {
        let mut info = SHELLEXECUTEINFOW {
            cbSize: mem::size_of::<SHELLEXECUTEINFOW>() as u32,
            fMask: SEE_MASK_NOCLOSEPROCESS,
            hwnd: ptr::null_mut(),
            lpVerb: operation.as_ptr(),
            lpFile: file.as_ptr(),
            lpParameters: params.as_ptr(),
            lpDirectory: ptr::null(),
            nShow: SW_SHOW,
            hInstApp: ptr::null_mut(),
            ..SHELLEXECUTEINFOW::default()
        };

        let result = ShellExecuteExW(&mut info);

        if result != TRUE {
            return Err(io::Error::last_os_error());
        }

        match WaitForSingleObject(info.hProcess, INFINITE) {
            WAIT_OBJECT_0 => (),
            WAIT_ABANDONED => return Err(io::Error::other("wait abandoned")),
            WAIT_TIMEOUT => return Err(io::Error::other("wait timed out")),
            _ => return Err(io::Error::last_os_error()),
        }

        let result = GetExitCodeProcess(info.hProcess, &mut exit_code);

        if result != TRUE {
            return Err(io::Error::last_os_error());
        }

        Ok(exit_code as i32)
    }
}

fn encode_params<A>(args: A) -> io::Result<OsString>
where
    A: IntoIterator,
    A::Item: AsRef<OsStr>,
{
    let mut params = String::new();

    for arg in args {
        let arg = arg.as_ref();

        let arg = match arg.to_str() {
            Some(arg) => arg,
            None => {
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    "argument is not valid UTF-8",
                ));
            }
        };

        params.push(' ');

        if arg.is_empty() {
            params.push_str("\"\"");
        } else if arg.find(&[' ', '\t', '"'][..]).is_none() {
            params.push_str(arg);
        } else {
            params.push('"');

            for c in arg.chars() {
                match c {
                    '\\' => params.push_str("\\\\"),
                    '"' => params.push_str("\\\""),
                    c => params.push(c),
                }
            }

            params.push('"');
        }
    }

    Ok(OsString::from(params))
}
