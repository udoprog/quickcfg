fn main() {
    platform();
}

#[cfg(windows)]
fn platform() {
    println!("cargo:rustc-link-lib=msi");
}

#[cfg(not(windows))]
fn platform() {}
