//! The Agility SDK is opted into by exports on the exe (see main.rs); the
//! linker has to export them for D3D12 to find them on the binary.
fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        println!("cargo:rustc-link-arg-bins=/EXPORT:D3D12SDKVersion");
        println!("cargo:rustc-link-arg-bins=/EXPORT:D3D12SDKPath");
    }
}