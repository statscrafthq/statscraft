# statscraft-sav

This crate started as a copy of the Rust reader and writer of the `ambers` crate, version 0.4.7,
by Albert Li (https://github.com/albertxli/ambers), published under the MIT license. The original
license text with its copyright line is in [LICENSE-ambers](LICENSE-ambers). The Python bindings
were left out. From that copy on, Statscraft maintains the code here.

It reads and writes `.sav` files (uncompressed or bytecode-compressed) and `.zsav` files
(zlib-compressed). `statscraft-data` is its only user.
