use std::ffi::{OsStr, OsString};
use std::io;
use std::os::windows::ffi::OsStrExt as _;
use std::ptr;
use winapi::shared::minwindef::DWORD;
use winapi::shared::minwindef::TRUE;
use winapi::shared::winerror::WAIT_TIMEOUT;
use winapi::um::processthreadsapi::GetExitCodeProcess;
use winapi::um::shellapi;
use winapi::um::synchapi::WaitForSingleObject;
use winapi::um::winbase::{INFINITE, WAIT_ABANDONED, WAIT_OBJECT_0};
use winapi::um::winuser;

fn wide_string(s: impl AsRef<OsStr>) -> Vec<u16> {
    s.as_ref().encode_wide().chain(Some(0)).collect::<Vec<_>>()
}

pub fn runas(command: crate::Command) -> io::Result<i32> {
    let operation: Vec<u16> = OsStr::new("runas\0").encode_wide().collect();

    let file = wide_string(&command.name);
    let params = wide_string(encode_params(&command.args)?);

    let mut exit_code = 0;

    unsafe {
        let mut info = shellapi::SHELLEXECUTEINFOW::default();

        info.cbSize = std::mem::size_of::<shellapi::SHELLEXECUTEINFOW>() as DWORD;
        info.fMask = shellapi::SEE_MASK_NOCLOSEPROCESS;
        info.hwnd = ptr::null_mut();
        info.lpVerb = operation.as_ptr();
        info.lpFile = file.as_ptr();
        info.lpParameters = params.as_ptr();
        info.lpDirectory = ptr::null();
        info.nShow = winuser::SW_SHOW;
        info.hInstApp = ptr::null_mut();

        let result = shellapi::ShellExecuteExW(&mut info);

        if result != TRUE {
            return Err(io::Error::last_os_error());
        }

        match WaitForSingleObject(info.hProcess, INFINITE) {
            WAIT_OBJECT_0 => (),
            WAIT_ABANDONED => return Err(io::Error::new(io::ErrorKind::Other, "wait abandoned")),
            WAIT_TIMEOUT => return Err(io::Error::new(io::ErrorKind::Other, "wait timed out")),
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
                ))
            }
        };

        params.push(' ');

        if arg.len() == 0 {
            params.push_str("\"\"");
        } else if arg.find(&[' ', '\t', '"'][..]).is_none() {
            params.push_str(&arg);
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
