# ahda
Compression, format conversion, and set operations for pseudoalignment data.

ahda is a WIP rewrite of
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
ahda *shall* support five main operations:
- `ahda cat` concatenate or convert compressed data.
- `ahda convert` convert plain text pseudoalignment data.
- `ahda decode` decompress pseudoalignment data.
- `ahda encode` compress pseudoalignment data.
- `ahda set` set operations on compressed data.

The following plain text formats *shall* be supported:
- ahda-tsv
- [Bifrost](https://github.com/pmelsted/bifrost)
- [Fulgor](https://github.com/jermp/fulgor)
- [Metagraph](https://github.com/ratschlab/metagraph)
- [SAM](https://samtools.github.io/hts-specs/SAMv1.pdf)
- [Themisto](https://github.com/algbio/themisto)

## License
ahda is dual-licensed under the [MIT](LICENSE-MIT) and [Apache 2.0](LICENSE-APACHE) licenses.
