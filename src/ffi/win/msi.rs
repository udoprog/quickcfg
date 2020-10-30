use std::ffi::OsStr;
use std::io;
use std::os::windows::ffi::OsStrExt as _;
use std::ptr;
use winapi::shared::{
    minwindef::{DWORD, LPDWORD, UINT},
    ntdef::{LPCWSTR, LPWSTR, WCHAR},
    winerror,
};

winapi::ENUM! {enum MSIINSTALLCONTEXT {
    MSIINSTALLCONTEXT_FIRSTVISIBLE   =  0,  // product visible to the current user
    MSIINSTALLCONTEXT_NONE           =  0,  // Invalid context for a product
    MSIINSTALLCONTEXT_USERMANAGED    =  1,  // user managed install context
    MSIINSTALLCONTEXT_USERUNMANAGED  =  2,  // user non-managed context
    MSIINSTALLCONTEXT_MACHINE        =  4,  // per-machine context
    MSIINSTALLCONTEXT_ALL            =  0b111, // All contexts. OR of all valid values
    MSIINSTALLCONTEXT_ALLUSERMANAGED =  8,  // all user-managed contexts
}}

extern "system" {
    fn MsiEnumProductsExW(
        szProductCode: LPCWSTR,
        szUserSid: LPCWSTR,
        dwContext: DWORD,
        dwIndex: DWORD,
        szInstalledProductCode: *mut [WCHAR; 39],
        pdwInstalledContext: *mut MSIINSTALLCONTEXT,
        szSid: LPWSTR,
        pcchSid: LPDWORD,
    ) -> UINT;

    fn MsiGetProductInfoExW(
        szProductCode: LPCWSTR,
        szUserSid: LPCWSTR,
        dwContext: MSIINSTALLCONTEXT,
        szProperty: LPCWSTR,
        szValue: LPWSTR,
        pcchValue: LPDWORD,
    ) -> UINT;
}

#[derive(Debug)]
pub struct MsiPackage {
    pub guid: String,
    pub name: String,
    pub version: String,
}

pub fn msi_get_product_info(
    product_code: &str,
    context: u32,
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
        winerror::ERROR_UNKNOWN_PRODUCT => return Ok(None),
        winerror::ERROR_UNKNOWN_PROPERTY => return Ok(None),
        errno => return Err(io::Error::from_raw_os_error(errno as i32)),
    }

    let value = String::from_utf16(&value[..(value_len as usize)])
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    Ok(Some(value))
}

pub fn msi_enum_products() -> io::Result<Vec<MsiPackage>> {
    let mut packages = Vec::new();

    for index in 0.. {
        let mut name = [0u16; 39];
        let mut context = MSIINSTALLCONTEXT_NONE;
        let mut sid = vec![0u16; 128];
        let mut sid_size = 128;

        let out = unsafe {
            MsiEnumProductsExW(
                ptr::null(),
                ptr::null(),
                MSIINSTALLCONTEXT_ALL,
                index,
                &mut name,
                &mut context,
                sid.as_mut_ptr(),
                &mut sid_size,
            )
        };

        match out {
            0 => (),
            winerror::ERROR_NO_MORE_ITEMS => break,
            errno => return Err(io::Error::from_raw_os_error(errno as i32)),
        }

        let product_code =
            String::from_utf16(&name[..38]).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

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
