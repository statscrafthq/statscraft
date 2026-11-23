# Contributing to Statscraft

## Build from source

You need Rust 1.95 or newer (`rustc --version` shows yours; `rustup update stable` gets a newer one).

```
git clone https://github.com/statscrafthq/statscraft
cd statscraft
cargo build --release -p statscraft -p statscraft-gui
```

The programs are then in `target/release/`: `statscraft`, the command line, and `statscraft-gui`, the window. Put `target/release` on your `PATH`, or call them by their full path. `cargo run -p statscraft-gui -- survey.sav job.sps` starts the window from the checkout with a data file and a syntax file.

A copy built from source does not update itself: pull and build again.
