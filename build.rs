//! Embeds the app icon into bocchi.exe, so it shows in File Explorer and on shortcuts.
fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon("assets/icon.ico");
        if let Err(e) = resource.compile() {
            // Not fatal: the window still gets its icon at runtime.
            println!("cargo:warning=Couldn't embed the exe icon: {e}");
        }
    }
}
