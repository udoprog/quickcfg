use core::mem::MaybeUninit;

use std::ffi::OsStr;
use std::io;
use std::os::windows::ffi::OsStrExt as _;
use std::ptr;

use windows_sys::Win32::Foundation::{
    ERROR_NO_MORE_ITEMS, ERROR_UNKNOWN_PRODUCT, ERROR_UNKNOWN_PROPERTY,
};
use windows_sys::Win32::System::ApplicationInstallationAndServicing::MSIINSTALLCONTEXT_ALL;
use windows_sys::Win32::System::ApplicationInstallationAndServicing::MsiEnumProductsExW;
use windows_sys::Win32::System::ApplicationInstallationAndServicing::MsiGetProductInfoExW;

#[derive(Debug)]
pub struct MsiPackage {
    pub guid: String,
    pub name: String,
    pub version: String,
}

pub fn msi_get_product_info(
    product_code: &str,
    context: i32,
    property: &str,
) -> io::Result<Option<String>> {
    let product_code = OsStr::new(product_code)
        .encode_wide()
        .chain(Some(0u16))
        .collect::<Vec<u16>>();

    let property = OsStr::new(property)
        .encode_wide()
        .chain(Some(0u16))
        .collect::<Vec<u16>>();

    let mut value = vec![0u16; 256];
    let mut value_len = value.len() as u32;

    let out = unsafe {
        MsiGetProductInfoExW(
            product_code.as_ptr(),
            ptr::null(),
            context,
            property.as_ptr(),
            value.as_mut_ptr(),
            &mut value_len,
        )
    };

    match out {
        0 => (),
        ERROR_UNKNOWN_PRODUCT => return Ok(None),
        ERROR_UNKNOWN_PROPERTY => return Ok(None),
        errno => return Err(io::Error::from_raw_os_error(errno as i32)),
    }

    let value =
        String::from_utf16(&value[..(value_len as usize)]).map_err(|e| io::Error::other(e))?;
    Ok(Some(value))
}

pub fn msi_enum_products() -> io::Result<Vec<MsiPackage>> {
    let mut packages = Vec::new();

    for index in 0.. {
        let mut name = [0u16; 39];
        let mut context = MaybeUninit::uninit();
        let mut sid = vec![0u16; 128];
        let mut sid_size = 128;

        let out = unsafe {
            MsiEnumProductsExW(
                ptr::null(),
                ptr::null(),
                MSIINSTALLCONTEXT_ALL as u32,
                index,
                name.as_mut_ptr(),
                context.as_mut_ptr(),
                sid.as_mut_ptr(),
                &mut sid_size,
            )
        };

        match out {
            0 => (),
            ERROR_NO_MORE_ITEMS => break,
            errno => return Err(io::Error::from_raw_os_error(errno as i32)),
        }

        let context = unsafe { context.assume_init() };
        let product_code = String::from_utf16(&name[..38]).map_err(|e| io::Error::other(e))?;

        let name = msi_get_product_info(&product_code, context, "PackageName")?;
        let version = msi_get_product_info(&product_code, context, "VersionString")?;

        if let (Some(name), Some(version)) = (name, version) {
            packages.push(MsiPackage {
                guid: product_code,
                name,
                version,
            });
        }
    }

    Ok(packages)
}
