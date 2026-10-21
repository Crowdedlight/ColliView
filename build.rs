//! Embeds the app icon in the Windows executable, for Explorer, shortcuts and pinned
//! taskbar entries. The running window's icon is set in `main.rs`.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=assets/colliview.ico");
    // The version info winresource embeds comes from Cargo.toml.
    println!("cargo:rerun-if-changed=Cargo.toml");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/colliview.ico");
        res.compile().expect("embedding the Windows icon failed");
    }
}
