# ahda
Compress pseudoalignment data and convert between different formats.

(WIP) ahda is a rewrite of
[alignment-writer](https://github.com/tmaklin/alignment-writer) dev branch in
Rust.

## Installation
``` sh
git clone https://github.com/tmaklin/ahda
cd ahda
cargo build --release
```
The built binary is located at `target/release/ahda`.

## About
ahda *shall** support three main operations:

- `ahda encode` compress pseudoalignment data from supported tools.
- `ahda decode` decompress the output from encode.
- `ahda cat` conversion between supported formats.

## License
ahda is dual-licensed under the [MIT](LICENSE-MIT) and [Apache 2.0](LICENSE-APACHE) licenses.
