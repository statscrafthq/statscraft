// Puts the icon and the program name into the Windows executable, for Explorer and the taskbar.
fn main() {
    println!("cargo:rerun-if-changed=icons/statscraft.ico");
    let windows_target = std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows");
    // a check for Windows from another system has no resource compiler; the real build runs on Windows
    if windows_target && !cfg!(windows) {
        println!("cargo:warning=not embedding the Windows icon when building on another system");
        return;
    }
    if windows_target {
        winresource::WindowsResource::new()
            .set_icon("icons/statscraft.ico")
            // Windows shows the file description as the program name, for example in Open with
            .set("FileDescription", "Statscraft")
            .set("ProductName", "Statscraft")
            .compile()
            .expect("embedding the Windows icon needs the Windows SDK resource compiler");
    }
}
