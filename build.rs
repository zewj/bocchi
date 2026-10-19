//! Picks the app icon and embeds it into the exe.
//!
//! If `assets/icon.png` / `assets/icon.ico` exist (your own picture, kept out of git), they're
//! used. Otherwise the original `assets/icon-default.*` is used, so a fresh clone still builds.
use std::path::Path;

fn main() {
    for file in ["assets/icon.png", "assets/icon.ico", "assets/icon-default.png", "assets/icon-default.ico"] {
        println!("cargo:rerun-if-changed={file}");
    }
    let pick = |name: &str| {
        let custom = format!("assets/icon.{name}");
        if Path::new(&custom).exists() { custom } else { format!("assets/icon-default.{name}") }
    };

    // The window icon: copied to a fixed spot so the code can include it.
    let out_dir = std::env::var("OUT_DIR").expect("cargo sets OUT_DIR");
    std::fs::copy(pick("png"), Path::new(&out_dir).join("app_icon.png")).expect("copy the icon");

    // The exe's own icon (File Explorer, shortcuts).
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon(&pick("ico"));
        if let Err(e) = resource.compile() {
            // Not fatal: the window still gets its icon at runtime.
            println!("cargo:warning=Couldn't embed the exe icon: {e}");
        }
    }
}
