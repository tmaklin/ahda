// ahda: Pseudoalignment compression and conversion between formats.
//
// Copyright 2025 Tommi Mäklin [tommi@maklin.fi].
//
// Copyrights in this project are retained by contributors. No copyright assignment
// is required to contribute to this project.
//
// Except as otherwise noted (below and/or in individual files), this
// project is licensed under the Apache License, Version 2.0
// <LICENSE-APACHE> or <http://www.apache.org/licenses/LICENSE-2.0> or
// the MIT license, <LICENSE-MIT> or <http://opensource.org/licenses/MIT>,
// at your option.
//

//! Encoder implementation for an iterator over [PseudoAln] records.
//!
//! Implements the [Encoder] struct that can be used to encode data from any
//! other struct that implements an iterator returning [PseudoAln] data.
//!
//! Calling next on Encoder will return a single block consisting of encoded
//! bytes representing all records in the block.
//!
//! To create a valid .ahda record, [Encoder::encode_header_and_flags] should be
//! called first and its output included as the first bytes in the record. This
//! method encodes the [FileHeader] and [FileFlags] corresponding to the data
//! stored in the Encoder.
//!
//! Block size can be controlled using [Encoder::set_block_size]. Larger blocks may
//! result in better compression ratios but require more memory to encode and
//! decode.
//!
//! ## Usage
//!
//! ### Encoding plain text data
//!
//! Create a [Parser](ahda::parser::Parser) on the plaintext input and pass it to Encoder
//!
//! ```rust
//! use ahda::encoder::Encoder;
//! use ahda::parser::Parser;
//! use ahda::{decode_from_read, PseudoAln};
//! use std::io::{Cursor, Seek, Write};
//!
//! // Mock inputs that will be store in FileHeader and FileFlags
//! let targets = vec!["chr.fasta".to_string(), "plasmid.fasta".to_string(), "virus.fasta".to_string()];
//! let queries = vec!["r1".to_string(), "r2".to_string(), "r651903".to_string(), "r7543".to_string(), "r16".to_string()];
//! let name = "sample".to_string();
//!
//! // Have this Metagraph input data:
//! //   3    r7543    chr.fasta:virus.fasta
//! //   0    r1       virus.fasta
//! //   4    r16      chr.fasta:plasmid.fasta:virus.fasta
//! //   2    r651903
//! //
//! let mut plaintext: Vec<u8> = Vec::new();
//! plaintext.append(&mut b"0\tr1\tvirus.fasta\n".to_vec());
//! plaintext.append(&mut b"3\tr7543\tchr.fasta:virus.fasta\n".to_vec());
//! plaintext.append(&mut b"4\tr16\tchr.fasta:plasmid.fasta:virus.fasta\n".to_vec());
//! plaintext.append(&mut b"2\tr651903\t\n".to_vec());
//!
//! let mut input: Cursor<Vec<u8>> = Cursor::new(plaintext.clone());
//!
//! // Create a Parser to convert the plain text data to PseudoAln and initialize Encoder on this parser to encode it
//! let mut parser = Parser::new(&mut input, &targets, &queries, &name).unwrap();
//! let mut encoder = Encoder::new(&mut parser, &targets, &queries, &name);
//! encoder.set_block_size(3);
//!
//! let mut output: Cursor<Vec<u8>> = Cursor::new(Vec::new());
//!
//! // Encode the file header and flags
//! let bytes = encoder.encode_header_and_flags().unwrap();
//! output.write_all(&bytes).unwrap();
//!
//! // Iterate over `encoder` to get the 2 encoded blocks
//! for block in encoder.by_ref() {
//!     output.write_all(&block).unwrap();
//! }
//!
//! // The alignments can be decoded from `output`
//! output.rewind();
//! let (_file_header, _file_flags, alns) = decode_from_read(&mut output).unwrap();
//!
//! assert_eq!(alns[0], PseudoAln { ones: Some(vec![2]), ones_names: Some(vec!["virus.fasta".to_string()]), query_id: Some(0), query_name: Some("r1".to_string()) });
//! assert_eq!(alns[1], PseudoAln { ones: Some(vec![0, 2]), ones_names: Some(vec!["chr.fasta".to_string(), "virus.fasta".to_string()]), query_id: Some(3), query_name: Some("r7543".to_string()) });
//! assert_eq!(alns[2], PseudoAln { ones: Some(vec![0, 1, 2]), ones_names: Some(vec!["chr.fasta".to_string(), "plasmid.fasta".to_string(), "virus.fasta".to_string()]), query_id: Some(4), query_name: Some("r16".to_string()) });
//! assert_eq!(alns[3], PseudoAln { ones: Some(vec![]), ones_names: Some(vec![]), query_id: Some(2), query_name: Some("r651903".to_string()) });
//! assert_eq!(alns.len(), 4);
//! ```
//!
//! ### Encoding alignments stored in memory
//!
//! Create an iterator over some container storing [PseudoAln](ahda::PseudoAln) and pass it to a new Encoder
//!
//! ```rust
//! use ahda::encoder::Encoder;
//! use ahda::{decode_from_read, PseudoAln};
//! use std::io::{Cursor, Seek, Write};
//!
//! let targets = vec!["chr.fasta".to_string(), "plasmid.fasta".to_string(), "virus.fasta".to_string()];
//! let queries = vec!["r1".to_string(), "r2".to_string(), "r651903".to_string(), "r7543".to_string(), "r16".to_string()];
//! let name = "sample".to_string();
//!
//! let data: Vec<PseudoAln> = vec![
//!                                 PseudoAln { ones: Some(vec![2]), ones_names: Some(vec!["virus.fasta".to_string()]), query_id: Some(0), query_name: Some("r1".to_string()) },
//!                                 PseudoAln { ones: Some(vec![0, 2]), ones_names: Some(vec!["chr.fasta".to_string(), "virus.fasta".to_string()]), query_id: Some(3), query_name: Some("r7543".to_string()) },
//!                                 PseudoAln { ones: Some(vec![0, 1, 2]), ones_names: Some(vec!["chr.fasta".to_string(), "plasmid.fasta".to_string(), "virus.fasta".to_string()]), query_id: Some(4), query_name: Some("r16".to_string()) },
//!                                 PseudoAln { ones: Some(vec![]), ones_names: Some(vec![]), query_id: Some(2), query_name: Some("r651903".to_string()) }
//!                                ];
//!
//! let mut iter = data.into_iter(); // Encoder::new expects PseudoAln and doesn't work on &PseudoAln
//! let mut encoder = Encoder::new(&mut iter, &targets, &queries, &name);
//!
//! // Encode the file header and flags
//! let bytes = encoder.encode_header_and_flags().unwrap();
//! let mut output: Cursor<Vec<u8>> = Cursor::new(Vec::new());
//! output.write_all(&bytes).unwrap();
//!
//! // Iterate over `encoder` to get the 2 encoded blocks
//! for block in encoder.by_ref() {
//!     output.write_all(&block).unwrap();
//! }
//!
//! // The alignments can be decoded from `output`
//! output.rewind();
//! let (_file_header, _file_flags, alns) = decode_from_read(&mut output).unwrap();
//!
//! assert_eq!(alns[0], PseudoAln { ones: Some(vec![2]), ones_names: Some(vec!["virus.fasta".to_string()]), query_id: Some(0), query_name: Some("r1".to_string()) });
//! assert_eq!(alns[1], PseudoAln { ones: Some(vec![0, 2]), ones_names: Some(vec!["chr.fasta".to_string(), "virus.fasta".to_string()]), query_id: Some(3), query_name: Some("r7543".to_string()) });
//! assert_eq!(alns[2], PseudoAln { ones: Some(vec![0, 1, 2]), ones_names: Some(vec!["chr.fasta".to_string(), "plasmid.fasta".to_string(), "virus.fasta".to_string()]), query_id: Some(4), query_name: Some("r16".to_string()) });
//! assert_eq!(alns[3], PseudoAln { ones: Some(vec![]), ones_names: Some(vec![]), query_id: Some(2), query_name: Some("r651903".to_string()) });
//! assert_eq!(alns.len(), 4);
//! ```
//!

pub mod bitmap_encoder;
pub mod pack_roaring;

use crate::PseudoAln;
use crate::headers::file::FileHeader;
use crate::headers::file::FileFlags;
use crate::headers::file::encode_file_header;
use crate::headers::file::encode_file_flags;
use pack_roaring::pack_block_roaring;

pub struct Encoder<'a, I: Iterator> where I: Iterator<Item=PseudoAln> {
    // Inputs
    records: &'a mut I,

    // These are given as construtor parameters
    header: FileHeader,
    flags: FileFlags,

    // Internals
    block_size: usize,
    blocks_written: usize,
}

impl<'a, I: Iterator> Encoder<'a, I> where I: Iterator<Item=PseudoAln> {
    pub fn new(
        records: &'a mut I,
        targets: &[String],
        queries: &[String],
        sample_name: &str,
    ) -> Self {
        let flags = FileFlags{ target_names: targets.to_vec(), query_name: sample_name.to_string() };
        let flags_bytes = crate::headers::file::encode_file_flags(&flags).unwrap();
        let header = FileHeader{ n_targets: targets.len() as u32, n_queries: queries.len() as u32, flags_len: flags_bytes.len() as u32, format: 1_u16, ph2: 0, ph3: 0, ph4: 0 };

        // Adjust block size to fit within 32-bit address space
        let block_size = ((u32::MAX as u64) / header.n_targets as u64).min(65537_u64) as usize;
        assert!(block_size > 1);
        let block_size = block_size - 1;

        Encoder{
            records,
            header, flags,
            block_size, blocks_written: 0_usize,
        }
    }
}

impl<I: Iterator> Encoder<'_, I> where I: Iterator<Item=PseudoAln> {
    pub fn encode_header_and_flags(
        &mut self,
    ) -> Option<Vec<u8>> {
        // TODO Replace unwraps in `encode_header_and_flags`
        let mut flags_bytes = encode_file_flags(&self.flags).unwrap();
        let mut header_bytes = encode_file_header(&self.header).unwrap();

        let mut out: Vec<u8> = Vec::new();
        out.append(&mut header_bytes);
        out.append(&mut flags_bytes);

        Some(out)
    }

    pub fn set_block_size(
        &mut self,
        block_size: usize
    ) {
        let new_block_size = block_size.min(65536_usize);
        assert!(new_block_size > 1);
        self.block_size = new_block_size;
    }

}

impl<I: Iterator> Iterator for Encoder<'_, I> where I: Iterator<Item=PseudoAln> {
    type Item = Vec<u8>;

    fn next(
        &mut self,
    ) -> Option<Vec<u8>> {
        let mut block_records: Vec<PseudoAln> = Vec::with_capacity(self.block_size);
        for record in self.records.by_ref() {
            // TODO Check that all fields are set?
            block_records.push(record);
            if block_records.len() == self.block_size {
                break;
            }
        }

        if block_records.is_empty() {
            return None
        }

        block_records.sort_by_key(|x| x.query_id.unwrap());

        let out = pack_block_roaring(&self.header, &block_records).unwrap();

        self.blocks_written += 1;

        Some(out)
    }

}

#[cfg(test)]
mod tests {

    #[test]
    fn encode_header_and_flags() {
        use crate::PseudoAln;
        use super::Encoder;

        let data = vec![
            PseudoAln{ones_names: Some(vec!["chr.fasta".to_string()]),  query_id: Some(1), ones: Some(vec![0]), query_name: Some("ERR4035126.2".to_string()) },
            PseudoAln{ones_names: Some(vec!["chr.fasta".to_string()]),  query_id: Some(0), ones: Some(vec![0]), query_name: Some("ERR4035126.1".to_string()) },
            PseudoAln{ones_names: Some(vec!["chr.fasta".to_string(), "plasmid.fasta".to_string()]),  query_id: Some(2), ones: Some(vec![0, 1]), query_name: Some("ERR4035126.651903".to_string()) },
            PseudoAln{ones_names: Some(vec![]),  query_id: Some(4), ones: Some(vec![]), query_name: Some("ERR4035126.16".to_string()) },
            PseudoAln{ones_names: Some(vec!["plasmid.fasta".to_string()]),  query_id: Some(3), ones: Some(vec![1]), query_name: Some("ERR4035126.7543".to_string()) },
        ];

        let expected = vec![2, 0, 0, 0, 5, 0, 0, 0, 36, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 10, 69, 82, 82, 52, 48, 51, 53, 49, 50, 54, 2, 9, 99, 104, 114, 46, 102, 97, 115, 116, 97, 13, 112, 108, 97, 115, 109, 105, 100, 46, 102, 97, 115, 116, 97];

        let targets = vec!["chr.fasta".to_string(), "plasmid.fasta".to_string()];
        let queries = vec!["ERR4035126.1".to_string(), "ERR4035126.2".to_string(), "ERR4035126.651903".to_string(), "ERR4035126.7543".to_string(), "ERR4035126.16".to_string()];
        let query_name ="ERR4035126".to_string();

        let mut tmp = data.into_iter();
        let mut encoder = Encoder::new(&mut tmp, &targets, &queries, &query_name);

        let got = encoder.encode_header_and_flags().unwrap();

        assert_eq!(got, expected);
    }

    #[test]
    fn next() {
        use crate::PseudoAln;
        use super::Encoder;

        let data = vec![
            PseudoAln{ones_names: Some(vec!["chr.fasta".to_string()]),  query_id: Some(1), ones: Some(vec![0]), query_name: Some("ERR4035126.2".to_string()) },
            PseudoAln{ones_names: Some(vec!["chr.fasta".to_string()]),  query_id: Some(0), ones: Some(vec![0]), query_name: Some("ERR4035126.1".to_string()) },
            PseudoAln{ones_names: Some(vec!["chr.fasta".to_string(), "plasmid.fasta".to_string()]),  query_id: Some(2), ones: Some(vec![0, 1]), query_name: Some("ERR4035126.651903".to_string()) },
            PseudoAln{ones_names: Some(vec![]),  query_id: Some(4), ones: Some(vec![]), query_name: Some("ERR4035126.16".to_string()) },
            PseudoAln{ones_names: Some(vec!["plasmid.fasta".to_string()]),  query_id: Some(3), ones: Some(vec![1]), query_name: Some("ERR4035126.7543".to_string()) },
        ];

        let expected: Vec<u8> = vec![5, 0, 0, 0, 103, 0, 0, 0, 40, 0, 0, 0, 63, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 99, 229, 113, 13, 10, 50, 49, 48, 54, 53, 52, 50, 211, 51, 68, 230, 24, 9, 34, 113, 204, 76, 13, 45, 13, 140, 249, 145, 68, 204, 77, 77, 140, 121, 145, 245, 154, 177, 50, 48, 50, 49, 179, 0, 0, 164, 198, 115, 218, 81, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 179, 50, 96, 96, 96, 100, 0, 1, 22, 6, 1, 48, 205, 196, 192, 194, 192, 202, 192, 206, 0, 0, 47, 109, 177, 38, 26, 0, 0, 0];

        let targets = vec!["chr.fasta".to_string(), "plasmid.fasta".to_string()];
        let queries = vec!["ERR4035126.1".to_string(), "ERR4035126.2".to_string(), "ERR4035126.651903".to_string(), "ERR4035126.7543".to_string(), "ERR4035126.16".to_string()];
        let query_name ="ERR4035126".to_string();

        let mut tmp = data.into_iter();
        let mut encoder = Encoder::new(&mut tmp, &targets, &queries, &query_name);
        encoder.set_block_size(1000);

        let got = encoder.next().unwrap();

        assert_eq!(encoder.next(), None);
        assert_eq!(got, expected);
    }

    #[test]
    fn encode_three_blocks_with_next() {
        use crate::PseudoAln;
        use super::Encoder;

        let data = vec![
            PseudoAln{ones_names: Some(vec!["chr.fasta".to_string()]),  query_id: Some(1), ones: Some(vec![0]), query_name: Some("ERR4035126.2".to_string()) },
            PseudoAln{ones_names: Some(vec!["chr.fasta".to_string()]),  query_id: Some(0), ones: Some(vec![0]), query_name: Some("ERR4035126.1".to_string()) },
            PseudoAln{ones_names: Some(vec!["chr.fasta".to_string(), "plasmid.fasta".to_string()]),  query_id: Some(2), ones: Some(vec![0, 1]), query_name: Some("ERR4035126.651903".to_string()) },
            PseudoAln{ones_names: Some(vec![]),  query_id: Some(4), ones: Some(vec![]), query_name: Some("ERR4035126.16".to_string()) },
            PseudoAln{ones_names: Some(vec!["plasmid.fasta".to_string()]),  query_id: Some(3), ones: Some(vec![1]), query_name: Some("ERR4035126.7543".to_string()) },
        ];

        let expected: Vec<u8> = vec![2, 0, 0, 0, 5, 0, 0, 0, 36, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 10, 69, 82, 82, 52, 48, 51, 53, 49, 50, 54, 2, 9, 99, 104, 114, 46, 102, 97, 115, 116, 97, 13, 112, 108, 97, 115, 109, 105, 100, 46, 102, 97, 115, 116, 97, 2, 0, 0, 0, 74, 0, 0, 0, 34, 0, 0, 0, 40, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 99, 226, 113, 13, 10, 50, 49, 48, 54, 53, 52, 50, 211, 51, 68, 230, 24, 49, 49, 48, 2, 0, 190, 252, 200, 192, 30, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 179, 50, 96, 96, 96, 100, 0, 1, 70, 6, 1, 48, 205, 196, 0, 0, 133, 36, 27, 152, 20, 0, 0, 0, 2, 0, 0, 0, 84, 0, 0, 0, 37, 0, 0, 0, 47, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 99, 18, 116, 13, 10, 50, 49, 48, 54, 53, 52, 50, 211, 51, 51, 53, 180, 52, 48, 230, 69, 18, 49, 52, 99, 98, 98, 1, 0, 241, 215, 115, 101, 36, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 179, 50, 96, 96, 96, 100, 0, 1, 70, 6, 1, 6, 6, 6, 22, 6, 86, 6, 0, 21, 37, 56, 88, 20, 0, 0, 0, 1, 0, 0, 0, 72, 0, 0, 0, 33, 0, 0, 0, 39, 0, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 99, 228, 119, 13, 10, 50, 49, 48, 54, 53, 52, 50, 211, 51, 55, 53, 49, 102, 100, 6, 0, 231, 180, 12, 70, 19, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 179, 50, 96, 96, 96, 100, 128, 0, 1, 6, 6, 6, 118, 6, 0, 71, 48, 17, 238, 18, 0, 0, 0];

        let targets = vec!["chr.fasta".to_string(), "plasmid.fasta".to_string()];
        let queries = vec!["ERR4035126.1".to_string(), "ERR4035126.2".to_string(), "ERR4035126.651903".to_string(), "ERR4035126.7543".to_string(), "ERR4035126.16".to_string()];
        let query_name ="ERR4035126".to_string();

        let mut tmp = data.into_iter();
        let mut encoder = Encoder::new(&mut tmp, &targets, &queries, &query_name);
        encoder.set_block_size(2);

        let mut got: Vec<u8> = Vec::new();
        got.append(&mut encoder.encode_header_and_flags().unwrap());
        for block in encoder.by_ref() {
            got.append(&mut block.clone());
        }

        assert_eq!(got, expected);
    }
}
