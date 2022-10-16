#[cfg(windows)]
fn main() {
    for p in quickcfg::ffi::win::msi_enum_products().unwrap() {
        println!("{:?}", p);
    }
}

#[cfg(not(windows))]
fn main() {}
