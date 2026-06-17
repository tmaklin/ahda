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
  - [Metagraph](https://github.com/ratschlab/metagraph) (`--query-mode labels` only)
  - [SAM](https://samtools.github.io/hts-specs/SAMv1.pdf) (input only)
  - [Themisto](https://github.com/algbio/themisto)

An additional custom plain text format meant to display all data contained in the records is also provided:
  - [Ahda .tsv](parser::ahda_tsv)

The default format for plain text outputs is Ahda .tsv.

See the documentation for more details.

### Command-line interface
The ahda CLI supports the following subcommands:
  - `ahda encode` compress pseudoalignment data from a supported format.
  - `ahda decode` decompress pseudoalignment data to a supported format.
  - `ahda convert` convert between supported plain text formats.
  - `ahda cat` concatenate binary data that doesn't contain duplicated queries.
  - `ahda set` perform set operations on compressed pseudoalignment data.

## License
ahda is dual-licensed under the [MIT](LICENSE-MIT) and [Apache 2.0](LICENSE-APACHE) licenses.
