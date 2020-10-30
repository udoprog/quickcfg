fn main() {
    platform();
}

#[cfg(windows)]
fn platform() {
    println!("cargo:rustc-link-lib=msi");

    /*let mut res = winres::WindowsResource::new();
    res.set_manifest(
        r#"
    <assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
    <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
        <security>
            <requestedPrivileges>
                <requestedExecutionLevel level="requireAdministrator" uiAccess="false" />
            </requestedPrivileges>
        </security>
    </trustInfo>
    </assembly>
    "#,
    );

    res.compile().expect("working resources");*/
}

#[cfg(not(windows))]
fn platform() {}
