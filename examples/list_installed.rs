fn main() {
    for p in quickcfg::ffi::win::msi_enum_products().unwrap() {
        println!("{:?}", p);
    }
}
