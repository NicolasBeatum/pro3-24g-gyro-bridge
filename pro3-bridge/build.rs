fn main() {
    println!("cargo:rerun-if-changed=../assets/app_icon.ico");

    // Embed the multi-size icon so Explorer / the taskbar show it for pro3-bridge.exe.
    // Checked via CARGO_CFG_TARGET_OS (not #[cfg]) so it follows the target, not the host.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        #[cfg(windows)]
        {
            let mut res = winresource::WindowsResource::new();
            res.set_icon("../assets/app_icon.ico");
            if let Err(e) = res.compile() {
                println!("cargo:warning=failed to embed app icon: {e}");
            }
        }
    }
}
