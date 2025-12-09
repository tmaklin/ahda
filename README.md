# ahda
Compression, format conversion, and set operations for pseudoalignment data.

## Installation
``` sh
git clone https://github.com/tmaklin/ahda
cd ahda
cargo build --release
```
The built binary is located at `target/release/ahda`.

## About
The following plain text formats are supported:
- [Bifrost](https://github.com/pmelsted/bifrost)
- [Fulgor](https://github.com/jermp/fulgor)
- [Metagraph](https://github.com/ratschlab/metagraph)
- [SAM](https://samtools.github.io/hts-specs/SAMv1.pdf)
- [Themisto](https://github.com/algbio/themisto)

### Command-line interface
The `ahda` CLI supports five subcommands:
- `ahda cat` concatenate compressed data.
- `ahda convert` convert plain text data to another format.
- `ahda decode` decompress pseudoalignment data.
- `ahda encode` compress pseudoalignment data.
- `ahda set` merge compressed data with set operations.

## License
ahda is dual-licensed under the [MIT](LICENSE-MIT) and [Apache 2.0](LICENSE-APACHE) licenses.
