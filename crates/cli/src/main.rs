fn main() {
    // shortcut: hand-rolled flag handling, switch to an argument parser when subcommands arrive (stage E1)
    match std::env::args().nth(1).as_deref() {
        Some("--version" | "-V") => println!("statscraft {}", env!("CARGO_PKG_VERSION")),
        _ => {
            eprintln!("usage: statscraft --version");
            std::process::exit(2);
        }
    }
}
