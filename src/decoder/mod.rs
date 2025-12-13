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

//! Decoder implementations from [Read] and bitmap iterators.
//!
//! Contains implementations for two core classes:
//!
//! - [Decoder]: reads the .ahda binary format from a connection implementing
//!   [Read] and returns blocks of [PseudoAln] records when [next] is called.
//!
//! - [BitmapDecoder](bitmap::BitmapDecoder): decodes a [PseudoAln] record from
//!   any struct that returns u64 indexes of aligned bits in a flattened
//!   pseudoalignment. Currently, the intended use case is with
//!   [RoaringBitmap](roaring::RoaringBitmap) or
//!   [RoaringTreemap](roaring::RoaringTreemap) but in principle works with
//!   other structs that implement a similar iterator.
//!
//! Internally, Decoder reads in a single block at a time and uses BitmapDecoder
//! to retrieve the alignments.
//!
//! BitmapDecoder will only return alignments that have some hits.
//!
//! Decoder will pad the output from BitmapDecoder to include queries that are
//! included in [BlockFlags] but did not align against any target.
//!
//! ## Usage
//!
//! ### Decoder
//!
//! Decoder is useful for reading all alignments or a block of alignments from a stream.
//!
//! ```rust
//! use ahda::{encode_from_read_to_write, decode_from_read_to_write};
//! use ahda::{Format, PseudoAln};
//! use ahda::decoder::Decoder;
//! use std::io::{Cursor, Seek};
//!
//! // First set up some mock encoded data
//! let targets = vec!["chr.fasta".to_string(), "plasmid.fasta".to_string(), "virus.fasta".to_string()];
//! let queries = vec!["r1".to_string(), "r2".to_string(), "r651903".to_string(), "r7543".to_string(), "r16".to_string()];
//! let name = "sample".to_string();
//!
//! // Have this input data:
//! //   3    r7543    chr.fasta:virus.fasta
//! //   0    r1       virus.fasta
//! //   4    r16      chr.fasta:plasmid.fasta:virus.fasta
//! //   2    r651903
//! //
//! let mut input_bytes: Vec<u8> = Vec::new();
//! input_bytes.append(&mut b"0\tr1\tvirus.fasta\n".to_vec());
//! input_bytes.append(&mut b"3\tr7543\tchr.fasta:virus.fasta\n".to_vec());
//! input_bytes.append(&mut b"4\tr16\tchr.fasta:plasmid.fasta:virus.fasta\n".to_vec());
//! input_bytes.append(&mut b"2\tr651903\t\n".to_vec());
//!
//! let mut input: Cursor<Vec<u8>> = Cursor::new(input_bytes.clone());
//!
//! let mut output: Cursor<Vec<u8>> = Cursor::new(Vec::new());
//! encode_from_read_to_write(&targets, &queries, &name, &mut input, &mut output).unwrap();
//! output.rewind();
//!
//! // Then, create a Decoder from `output` and retrieve the original data
//! let mut decoder = Decoder::new(&mut output);
//!
//! let mut alns: Vec<PseudoAln> = Vec::new();
//! alns.extend(decoder); // Use Iterator to read all alignments from Decoder
//!
//! let expected = vec![
//!                     PseudoAln { ones: Some(vec![2]), ones_names: Some(vec!["virus.fasta".to_string()]), query_id: Some(0), query_name: Some("r1".to_string()) },
//!                     PseudoAln { ones: Some(vec![0, 2]), ones_names: Some(vec!["chr.fasta".to_string(), "virus.fasta".to_string()]), query_id: Some(3), query_name: Some("r7543".to_string()) },
//!                     PseudoAln { ones: Some(vec![0, 1, 2]), ones_names: Some(vec!["chr.fasta".to_string(), "plasmid.fasta".to_string(), "virus.fasta".to_string()]), query_id: Some(4), query_name: Some("r16".to_string()) },
//!                     PseudoAln { ones: Some(vec![]), ones_names: Some(vec![]), query_id: Some(2), query_name: Some("r651903".to_string()) }
//!                     ];
//!
//! assert_eq!(alns, expected);
//! ```
//!
//! ### BitmapDecoder
//!
//! BitmapDecoder is useful for retrieving the alignments stored in some bitmap
//! that can be iterated to get the indexes of aligned bits in a flattened
//! representation of the pseudoalignment.
//!
//! ```rust
//! use ahda::headers::file::{FileHeader, FileFlags};
//! use ahda::headers::block::{BlockHeader, BlockFlags};
//! use ahda::decoder::bitmap::BitmapDecoder;
//! use ahda::PseudoAln;
//! use roaring::RoaringBitmap;
//!
//! let input = RoaringBitmap::from([2, 9, 11, 12, 13, 14]);
//! // `input` could alternatively be any u32 container, eg. a vector: vec![2_u32, 9, 11, 12, 13, 14]
//!
//! let file_header = FileHeader { n_targets: 3, n_queries: 5, flags_len: 44, format: 1, bitmap_type: 0, ph3: 0, ph4: 0 };
//! let file_flags = FileFlags { query_name: "sample".to_string(), target_names: vec!["chr.fasta".to_string(), "plasmid.fasta".to_string(), "virus.fasta".to_string()] };
//! let block_header = BlockHeader { num_records: 4, deflated_len: 90, block_len: 28, flags_len: 27, start_idx: 0, placeholder2: 0, placeholder3: 0 };
//! let block_flags = BlockFlags { queries: vec!["r1".to_string(), "r651903".to_string(), "r7543".to_string(), "r16".to_string()], query_ids: vec![0, 2, 3, 4] };
//!
//! let mut bits_iter = input.iter().map(|x| x as u64); // BitmapDecoder expects u64 indices
//! let mut bitmap_decoder = BitmapDecoder::new(&mut bits_iter, file_header, file_flags, block_header, block_flags);
//!
//! assert_eq!(bitmap_decoder.next().unwrap(), PseudoAln { ones: Some(vec![2]), ones_names: Some(vec!["virus.fasta".to_string()]), query_id: Some(0), query_name: Some("r1".to_string()) });
//! assert_eq!(bitmap_decoder.next().unwrap(), PseudoAln { ones: Some(vec![0, 2]), ones_names: Some(vec!["chr.fasta".to_string(), "virus.fasta".to_string()]), query_id: Some(3), query_name: Some("r7543".to_string()) });
//! assert_eq!(bitmap_decoder.next().unwrap(), PseudoAln { ones: Some(vec![0, 1, 2]), ones_names: Some(vec!["chr.fasta".to_string(), "plasmid.fasta".to_string(), "virus.fasta".to_string()]), query_id: Some(4), query_name: Some("r16".to_string()) });
//!
//! assert_eq!(bitmap_decoder.next(), None); // Note that the PseudoAln with query_id: 2 is not included because it did not align against anything
//! ```
//!

pub mod bitmap;

use crate::PseudoAln;
use crate::headers::file::FileHeader;
use crate::headers::file::FileFlags;
use crate::headers::file::read_file_header;
use crate::headers::file::read_file_flags;
use crate::headers::block::BlockHeader;
use crate::headers::block::BlockFlags;
use crate::headers::block::read_block_header;
use crate::compression::BitmapType;
use crate::compression::roaring32::unpack_block_roaring32;
use crate::compression::roaring64::unpack_block_roaring64;

use std::collections::HashMap;
use std::collections::HashSet;
use std::io::Read;

type E = Box<dyn std::error::Error>;

// TODO Implement IntoIterator for Decoder

pub struct Decoder<'a, R: Read> {
    // Inputs
    conn: &'a mut R,

    header: FileHeader,
    flags: FileFlags,

    // Internals
    block_header: Option<BlockHeader>,
    block_flags: Option<BlockFlags>,
    block: Vec<PseudoAln>,
    block_index: usize,
}

impl<'a, R: Read> Decoder<'a, R> {
    pub fn new(
        conn: &'a mut R,
    ) -> Self {

        let header = read_file_header(conn).unwrap();
        let flags = read_file_flags(&header, conn).unwrap();

        Decoder{
            conn,
            header, flags,
            block_header: None, block_flags: None,
            block: Vec::new(), block_index: 0_usize,
        }
    }
}

impl<R: Read> Decoder<'_, R> {

    // TODO ugly copy paste in alns_from_roaring32 and alns_from_roaring64

    fn alns_from_roaring32(
        &mut self,
        bytes: &[u8],
    ) -> Result<Vec<PseudoAln>, E> {
        let (bitmap, block_flags) = unpack_block_roaring32(bytes, self.block_header.as_ref().unwrap())?;
        let mut name_to_id: HashMap<String, u32> = HashMap::with_capacity(self.block_header.as_ref().unwrap().num_records as usize);
        let mut seen: HashSet<u32> = HashSet::with_capacity(self.block_header.as_ref().unwrap().num_records as usize);
        block_flags.query_ids.iter().zip(block_flags.queries.iter()).for_each(|(idx, name)| {
            name_to_id.insert(name.clone(), *idx);
        });

        let mut tmp = bitmap.iter().map(|x| x as u64);
        let bitmap_decoder = bitmap::BitmapDecoder::new(&mut tmp, self.header.clone(), self.flags.clone(), self.block_header.as_ref().unwrap().clone(), block_flags.clone());
        let mut alns: Vec<PseudoAln> = Vec::new();
        for mut record in bitmap_decoder {
            let query_id = *name_to_id.get(record.query_name.as_ref().unwrap()).unwrap();
            record.query_id = Some(query_id);
            seen.insert(query_id);
            alns.push(record);
        }

        block_flags.query_ids.iter().zip(block_flags.queries.iter()).for_each(|(idx, name)| {
            if !seen.contains(idx) {
                alns.push(PseudoAln{ ones_names: Some(vec![]), query_id: Some(*idx), ones: Some(vec![]), query_name: Some(name.clone()) });
            }
        });

        self.block_flags = Some(block_flags);
        Ok(alns)
    }

    fn alns_from_roaring64(
        &mut self,
        bytes: &[u8],
    ) -> Result<Vec<PseudoAln>, E> {
        let (bitmap, block_flags) = unpack_block_roaring64(bytes, self.block_header.as_ref().unwrap())?;
        let mut name_to_id: HashMap<String, u32> = HashMap::with_capacity(self.block_header.as_ref().unwrap().num_records as usize);
        let mut seen: HashSet<u32> = HashSet::with_capacity(self.block_header.as_ref().unwrap().num_records as usize);
        block_flags.query_ids.iter().zip(block_flags.queries.iter()).for_each(|(idx, name)| {
            name_to_id.insert(name.clone(), *idx);
        });

        let mut tmp = bitmap.iter();
        let bitmap_decoder = bitmap::BitmapDecoder::new(&mut tmp, self.header.clone(), self.flags.clone(), self.block_header.as_ref().unwrap().clone(), block_flags.clone());
        let mut alns: Vec<PseudoAln> = Vec::new();
        for mut record in bitmap_decoder {
            let query_id = *name_to_id.get(record.query_name.as_ref().unwrap()).unwrap();
            record.query_id = Some(query_id);
            seen.insert(query_id);
            alns.push(record);
        }

        block_flags.query_ids.iter().zip(block_flags.queries.iter()).for_each(|(idx, name)| {
            if !seen.contains(idx) {
                alns.push(PseudoAln{ ones_names: Some(vec![]), query_id: Some(*idx), ones: Some(vec![]), query_name: Some(name.clone()) });
            }
        });

        self.block_flags = Some(block_flags);
        Ok(alns)
    }

    pub fn file_header(
        &self,
    ) -> &FileHeader {
        &self.header
    }

    pub fn file_flags(
        &self,
    ) -> &FileFlags {
        &self.flags
    }

    fn next_block(
        &mut self,
    ) -> Option<Vec<PseudoAln>> {
        match read_block_header(self.conn) {
            Ok(block_header) => {
                let mut bytes: Vec<u8> = vec![0; block_header.deflated_len as usize];
                self.conn.read_exact(&mut bytes).unwrap();
                self.block_header = Some(block_header);
                let alns = match BitmapType::from_u16(self.header.bitmap_type).unwrap() {
                    BitmapType::Roaring32 => {
                        self.alns_from_roaring32(&bytes).unwrap()
                    },
                    BitmapType::Roaring64 => {
                        self.alns_from_roaring64(&bytes).unwrap()
                    }
                };


                Some(alns)
            },
            _ => None,
        }
    }
}

impl<R: Read> Iterator for Decoder<'_, R> {
    type Item = PseudoAln;

    fn next(
        &mut self,
    ) -> Option<Self::Item> {
        if self.block_index < self.block.len() {
            self.block_index += 1;
            let ret = self.block[self.block_index - 1].clone();
            Some(ret)
        } else {
            self.block = self.next_block()?;
            self.block_index = 0;
            self.next()
        }
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn file_header_and_file_flags() {
        use super::Decoder;
        use crate::headers::file::FileFlags;
        use crate::headers::file::FileHeader;

        use std::io::Cursor;

        let expected_header = FileHeader { n_targets: 2, n_queries: 5, flags_len: 36, format: 1, bitmap_type: 0, ph3: 0, ph4: 0 };
        let expected_flags = FileFlags { query_name: "ERR4035126".to_string(), target_names: vec!["chr.fasta".to_string(), "plasmid.fasta".to_string()] };

        let data_bytes: Vec<u8> = vec![2, 0, 0, 0, 5, 0, 0, 0, 36, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 10, 69, 82, 82, 52, 48, 51, 53, 49, 50, 54, 2, 9, 99, 104, 114, 46, 102, 97, 115, 116, 97, 13, 112, 108, 97, 115, 109, 105, 100, 46, 102, 97, 115, 116, 97, 5, 0, 0, 0, 102, 0, 0, 0, 26, 0, 0, 0, 81, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 147, 239, 230, 96, 0, 131, 255, 155, 141, 18, 18, 18, 82, 24, 24, 197, 216, 24, 13, 206, 30, 57, 112, 232, 192, 169, 3, 39, 15, 156, 122, 44, 37, 146, 146, 148, 144, 147, 149, 145, 178, 44, 189, 229, 140, 161, 136, 203, 163, 25, 51, 165, 162, 164, 36, 62, 43, 121, 207, 254, 168, 252, 241, 140, 175, 111, 79, 164, 164, 228, 140, 136, 25, 140, 102, 251, 13, 119, 102, 51, 48, 48, 0, 0, 158, 168, 250, 0, 82, 0, 0, 0];
        let mut data: Cursor<Vec<u8>> = Cursor::new(data_bytes);

        let decoder = Decoder::new(&mut data);

        let got_header = decoder.file_header().clone();
        let got_flags = decoder.file_flags().clone();

        assert_eq!(got_header, expected_header);
        assert_eq!(got_flags, expected_flags);
    }

    #[test]
    fn next() {
        use super::Decoder;
        use crate::PseudoAln;

        use std::io::Cursor;

        let mut expected = vec![
            PseudoAln{ones_names: Some(vec!["chr.fasta".to_string()]),  query_id: Some(1), ones: Some(vec![0]), query_name: Some("ERR4035126.2".to_string()) },
            PseudoAln{ones_names: Some(vec!["chr.fasta".to_string()]),  query_id: Some(0), ones: Some(vec![0]), query_name: Some("ERR4035126.1".to_string()) },
            PseudoAln{ones_names: Some(vec!["chr.fasta".to_string(), "plasmid.fasta".to_string()]),  query_id: Some(2), ones: Some(vec![0, 1]), query_name: Some("ERR4035126.651903".to_string()) },
            PseudoAln{ones_names: Some(vec![]),  query_id: Some(4), ones: Some(vec![]), query_name: Some("ERR4035126.16".to_string()) },
            PseudoAln{ones_names: Some(vec!["plasmid.fasta".to_string()]),  query_id: Some(3), ones: Some(vec![1]), query_name: Some("ERR4035126.7543".to_string()) },
        ];
        expected.sort_by_key(|x| *x.query_id.as_ref().unwrap());

        let data_bytes: Vec<u8> = vec![2, 0, 0, 0, 5, 0, 0, 0, 36, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 10, 69, 82, 82, 52, 48, 51, 53, 49, 50, 54, 2, 9, 99, 104, 114, 46, 102, 97, 115, 116, 97, 13, 112, 108, 97, 115, 109, 105, 100, 46, 102, 97, 115, 116, 97, 5, 0, 0, 0, 103, 0, 0, 0, 40, 0, 0, 0, 63, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 99, 229, 113, 13, 10, 50, 49, 48, 54, 53, 52, 50, 211, 51, 68, 230, 24, 9, 34, 113, 204, 76, 13, 45, 13, 140, 249, 145, 68, 204, 77, 77, 140, 121, 145, 245, 154, 177, 50, 48, 50, 49, 179, 0, 0, 164, 198, 115, 218, 81, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 179, 50, 96, 96, 96, 100, 0, 1, 22, 6, 1, 48, 205, 196, 192, 194, 192, 202, 192, 206, 0, 0, 47, 109, 177, 38, 26, 0, 0, 0];
        let mut data: Cursor<Vec<u8>> = Cursor::new(data_bytes);

        let mut decoder = Decoder::new(&mut data);

        for i in 0..expected.len() {
            let got = decoder.next().unwrap();
            assert_eq!(got, expected[i]);
        }
        assert_eq!(decoder.next(), None);
    }

    #[test]
    fn next_block() {
        use super::Decoder;
        use crate::PseudoAln;

        use std::io::Cursor;

        let mut expected = vec![
            PseudoAln{ones_names: Some(vec!["chr.fasta".to_string()]),  query_id: Some(1), ones: Some(vec![0]), query_name: Some("ERR4035126.2".to_string()) },
            PseudoAln{ones_names: Some(vec!["chr.fasta".to_string()]),  query_id: Some(0), ones: Some(vec![0]), query_name: Some("ERR4035126.1".to_string()) },
            PseudoAln{ones_names: Some(vec!["chr.fasta".to_string(), "plasmid.fasta".to_string()]),  query_id: Some(2), ones: Some(vec![0, 1]), query_name: Some("ERR4035126.651903".to_string()) },
            PseudoAln{ones_names: Some(vec![]),  query_id: Some(4), ones: Some(vec![]), query_name: Some("ERR4035126.16".to_string()) },
            PseudoAln{ones_names: Some(vec!["plasmid.fasta".to_string()]),  query_id: Some(3), ones: Some(vec![1]), query_name: Some("ERR4035126.7543".to_string()) },
        ];
        expected.sort_by_key(|x| *x.query_id.as_ref().unwrap());

        let data_bytes: Vec<u8> = vec![2, 0, 0, 0, 5, 0, 0, 0, 36, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 10, 69, 82, 82, 52, 48, 51, 53, 49, 50, 54, 2, 9, 99, 104, 114, 46, 102, 97, 115, 116, 97, 13, 112, 108, 97, 115, 109, 105, 100, 46, 102, 97, 115, 116, 97, 5, 0, 0, 0, 103, 0, 0, 0, 40, 0, 0, 0, 63, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 99, 229, 113, 13, 10, 50, 49, 48, 54, 53, 52, 50, 211, 51, 68, 230, 24, 9, 34, 113, 204, 76, 13, 45, 13, 140, 249, 145, 68, 204, 77, 77, 140, 121, 145, 245, 154, 177, 50, 48, 50, 49, 179, 0, 0, 164, 198, 115, 218, 81, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 179, 50, 96, 96, 96, 100, 0, 1, 22, 6, 1, 48, 205, 196, 192, 194, 192, 202, 192, 206, 0, 0, 47, 109, 177, 38, 26, 0, 0, 0];
        let mut data: Cursor<Vec<u8>> = Cursor::new(data_bytes);

        let mut decoder = Decoder::new(&mut data);

        let mut got = decoder.next_block().unwrap();
        got.sort_by_key(|x| *x.query_id.as_ref().unwrap());

        assert_eq!(got, expected);
    }

    #[test]
    fn decode_three_blocks() {
        use super::Decoder;
        use crate::PseudoAln;

        use std::io::Cursor;

        let mut expected = vec![
            PseudoAln{ones_names: Some(vec!["chr.fasta".to_string()]),  query_id: Some(1), ones: Some(vec![0]), query_name: Some("ERR4035126.2".to_string()) },
            PseudoAln{ones_names: Some(vec!["chr.fasta".to_string()]),  query_id: Some(0), ones: Some(vec![0]), query_name: Some("ERR4035126.1".to_string()) },
            PseudoAln{ones_names: Some(vec!["chr.fasta".to_string(), "plasmid.fasta".to_string()]),  query_id: Some(2), ones: Some(vec![0, 1]), query_name: Some("ERR4035126.651903".to_string()) },
            PseudoAln{ones_names: Some(vec![]),  query_id: Some(4), ones: Some(vec![]), query_name: Some("ERR4035126.16".to_string()) },
            PseudoAln{ones_names: Some(vec!["plasmid.fasta".to_string()]),  query_id: Some(3), ones: Some(vec![1]), query_name: Some("ERR4035126.7543".to_string()) },
        ];
        expected.sort_by_key(|x| *x.query_id.as_ref().unwrap());

        let data_bytes: Vec<u8> = vec![2, 0, 0, 0, 5, 0, 0, 0, 36, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 10, 69, 82, 82, 52, 48, 51, 53, 49, 50, 54, 2, 9, 99, 104, 114, 46, 102, 97, 115, 116, 97, 13, 112, 108, 97, 115, 109, 105, 100, 46, 102, 97, 115, 116, 97, 2, 0, 0, 0, 74, 0, 0, 0, 34, 0, 0, 0, 40, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 99, 226, 113, 13, 10, 50, 49, 48, 54, 53, 52, 50, 211, 51, 68, 230, 24, 49, 49, 48, 2, 0, 190, 252, 200, 192, 30, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 179, 50, 96, 96, 96, 100, 0, 1, 70, 6, 1, 48, 205, 196, 0, 0, 133, 36, 27, 152, 20, 0, 0, 0, 2, 0, 0, 0, 88, 0, 0, 0, 39, 0, 0, 0, 49, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 99, 18, 116, 13, 10, 50, 49, 48, 54, 53, 52, 50, 211, 51, 51, 53, 180, 52, 48, 230, 71, 18, 49, 55, 53, 49, 102, 98, 98, 6, 0, 10, 60, 125, 12, 38, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 179, 50, 96, 96, 96, 100, 0, 1, 38, 6, 1, 6, 6, 6, 22, 6, 86, 6, 118, 6, 0, 163, 60, 183, 5, 22, 0, 0, 0, 1, 0, 0, 0, 61, 0, 0, 0, 24, 0, 0, 0, 37, 0, 0, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 99, 228, 117, 13, 10, 50, 49, 48, 54, 53, 52, 50, 211, 51, 52, 99, 100, 1, 0, 105, 171, 165, 101, 17, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 179, 50, 96, 0, 3, 0, 142, 53, 76, 217, 8, 0, 0, 0];
        let mut data: Cursor<Vec<u8>> = Cursor::new(data_bytes);

        let decoder = Decoder::new(&mut data);

        let mut got: Vec<PseudoAln> = Vec::new();
        got.extend(decoder);
        got.sort_by_key(|x| *x.query_id.as_ref().unwrap());

        assert_eq!(got, expected);
    }
}
