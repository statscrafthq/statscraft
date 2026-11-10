// Puts the icon and the program name into the Windows executable, for Explorer and the taskbar.
fn main() {
    println!("cargo:rerun-if-changed=icons/statscraft.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        winresource::WindowsResource::new()
            .set_icon("icons/statscraft.ico")
            // Windows shows the file description as the program name, for example in Open with
            .set("FileDescription", "StatsCraft")
            .set("ProductName", "StatsCraft")
            .compile()
            .expect("embedding the Windows icon needs the Windows SDK resource compiler");
    }
}
