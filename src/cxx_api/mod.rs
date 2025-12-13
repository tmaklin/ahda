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

//! C++ API for encoding and decoding .ahda records.
//!
//! The API operates on the set bit indexes of a *flattened pseudoalignment*,
//! which is a `num_queries * num_targets` vector of boolean values, where the
//! value (query_index, target_index) implies an alignment for the query
//! sequence with `query_index` against the target sequence with `target_index`.
//!
//! The flattened pseudoalignment is assumed to be stored in a query-major
//! format, meaning that the pseudoalignments bits of a single query sequence
//! are stored contiguously in memory.
//!
//! Currently, the API only supports a 32-bit address space, meaning that the
//! supported size of the input alignment is `num_queries * num_targets < 2^32`.
//!
//! **TODO** Support a larger address space.
//!
//! ## Usage
//!
//! **TODO** Usage examples for the C++ API.
//!

use crate::decode_from_read_to_roaring;
use crate::headers::file::encode_header_and_flags;
use crate::headers::file::build_header_and_flags;
use crate::headers::file::read_file_header_and_flags;
use crate::headers::block::read_block_header_and_flags;
use crate::encoder::bitmap_encoder::BitmapEncoder;
use crate::compression::roaring32::pack_block_roaring32;

use std::io::Cursor;

use cxx::CxxString;
use cxx::CxxVector;
use roaring::RoaringBitmap;

#[cxx::bridge(namespace = "ahda")]
mod ffi {

    extern "Rust" {
        fn encode_file_header_and_flags(
            targets: &CxxVector<CxxString>,
            queries: &CxxVector<CxxString>,
            name: &CxxString,
        ) -> Vec<u8>;

        fn encode_block(
            queries: &CxxVector<CxxString>,
            query_ids: &CxxVector<u32>,
            set_bits: &CxxVector<u32>,
        ) -> Vec<u8>;

        fn encode_bitmap(
            targets: &CxxVector<CxxString>,
            queries: &CxxVector<CxxString>,
            name: &CxxString,
            set_bits: &CxxVector<u32>,
        ) -> Vec<u8>;

        fn decode_bitmap(
            bytes: &CxxVector<u8>,
        ) -> Vec<u32>;

        fn decode_target_names(
            bytes: &CxxVector<u8>,
        ) -> Vec<String>;

        pub fn decode_query_ids(
            bytes: &CxxVector<u8>,
        ) -> Vec<u32>;

        pub fn decode_query_names(
            bytes: &CxxVector<u8>,
        ) -> Vec<String>;
}
}

/// Encode the file header and file flags bytes.
///
/// Calls [build_header_and_flags] on the input data and then creates the
/// encoded data by calling [encode_header_and_flags].
///
/// The output bytes should always be written at the start of the .ahda record.
///
pub fn encode_file_header_and_flags(
    targets: &CxxVector<CxxString>,
    queries: &CxxVector<CxxString>,
    name: &CxxString,
) -> Vec<u8> {
    let query_names: Vec<String> = queries.iter().map(|x| x.as_bytes().iter().map(|x| *x as char).collect::<String>()).collect();
    let target_names: Vec<String> = targets.iter().map(|x| x.as_bytes().iter().map(|x| *x as char).collect::<String>()).collect();
    let query_name: String = name.as_bytes().iter().map(|x| *x as char).collect::<String>();

    let (header, flags) = build_header_and_flags(&target_names, &query_names, &query_name).unwrap();
    let bytes: Vec<u8> = encode_header_and_flags(&header, &flags).unwrap();

    bytes
}

/// Encode a single .ahda block and its block header and flags.
///
/// Creates a [RoaringBitmap] from the set bit indexes and calls [pack_block_roaring] to
/// encode the block header, block flags, and block contents.
///
/// The output is a valid block record that can be appended to an .ahda record
/// containing the corresponding file header and flags.
///
pub fn encode_block(
    queries: &CxxVector<CxxString>,
    query_ids: &CxxVector<u32>,
    set_bits: &CxxVector<u32>,
) -> Vec<u8> {
    let bitmap = RoaringBitmap::from_iter(set_bits.iter());
    let query_names: Vec<String> = queries.iter().map(|x| x.as_bytes().iter().map(|x| *x as char).collect::<String>()).collect();
    let block = pack_block_roaring32(&query_names, query_ids.as_slice(), &bitmap);
    block.unwrap()
}

/// Encode a complete .ahda record from the set bits in a flattened pseudoalignment.
///
/// Creates an iterator over the set bit indexes and uses a [BitmapEncoder] to
/// encode a valid .ahda record.
///
/// The output is a complete .ahda record that contains the file header, file
/// flags, and all block data required to store the alignment. This can be
/// written to a file without further API calls.
///
pub fn encode_bitmap(
    targets: &CxxVector<CxxString>,
    queries: &CxxVector<CxxString>,
    name: &CxxString,
    set_bits: &CxxVector<u32>,
) -> Vec<u8> {
    let query_names: Vec<String> = queries.iter().map(|x| x.as_bytes().iter().map(|x| *x as char).collect::<String>()).collect();
    let target_names: Vec<String> = targets.iter().map(|x| x.as_bytes().iter().map(|x| *x as char).collect::<String>()).collect();
    let query_name: String = name.as_bytes().iter().map(|x| *x as char).collect::<String>();

    let mut set_bits_iter = set_bits.as_slice().iter().map(|x| *x as u64);
    let mut encoder = BitmapEncoder::new(&mut set_bits_iter, &target_names, &query_names, &query_name);

    let mut bytes: Vec<u8> = encoder.encode_header_and_flags().unwrap();
    for mut block in encoder.by_ref() {
        bytes.append(&mut block);
    }

    bytes
}

/// Decodes the indexes of set bits in a flattened pseudoalignment from an .ahda record.
///
/// Calls [decode_from_read_to_roaring] to extract a [RoaringBitmap]
/// representing the full pseudoalignment record contained in the .ahda record.
///
/// The input should contain the full contents of the .ahda record to be decoded.
///
/// The output is a vector containing the indexes of set bits, ie.
/// pseudoalignments, in a flattened pseudoalignment.
///
pub fn decode_bitmap(
    bytes: &CxxVector<u8>,
) -> Vec<u32> {
    let mut cursor = Cursor::new(bytes.as_slice());
    let (bitmap, _, _, _) = decode_from_read_to_roaring(&mut cursor).unwrap();
    let set_bits: Vec<u32> = bitmap.iter().collect();
    set_bits
}

/// Decodes the target sequence names from the file flags of an .ahda record.
///
/// The input should contain at least the bytes representing the [FileHeader]
/// and [FileFlags] in the .ahda record.
///
/// The output is a vector with the names of the target sequences. The position
/// of each element in the vector corresponds to its index in the
/// pseudoalignment.
///
pub fn decode_target_names(
    bytes: &CxxVector<u8>,
) -> Vec<String> {
    let mut cursor = Cursor::new(bytes.as_slice());
    let (header, flags) = read_file_header_and_flags(&mut cursor).unwrap();
    assert_eq!(header.n_targets as usize, flags.target_names.len());
    flags.target_names
}

/// Decodes the query sequence names from the block flags in an .ahda record.
///
/// The input should contain a complete .ahda record with the file header and
/// flags and all block data.
///
/// The output is a vector with the names of the query sequences. The position
/// of each element in the vector does **not** necessarily correspond to its
/// index in the original data, use [decode_query_ids] to get this information.
///
pub fn decode_query_names(
    bytes: &CxxVector<u8>,
) -> Vec<String> {
    let mut cursor = Cursor::new(bytes.as_slice());
    let (header, _) = read_file_header_and_flags(&mut cursor).unwrap();

    let mut query_names: Vec<String> = Vec::with_capacity(header.n_queries as usize);
    while let Ok((header, mut flags)) = read_block_header_and_flags(&mut cursor) {
        assert_eq!(header.num_records as usize, query_names.len());
        assert_eq!(flags.query_ids.len(), query_names.len());
        query_names.append(&mut flags.queries);
        cursor.set_position(cursor.position() + header.block_len as u64);
    }

    query_names
}

/// Decodes the query sequence indexes from the block flags in an .ahda record.
///
/// The input should contain a complete .ahda record with the file header and
/// flags and all block data.
///
/// The output is a vector with the indexes of the query sequences in the
/// original query sequence file. Use [decode_query_names] to get the vector
/// containing the names that can be indexed by the output from
/// decode_query_ids.
///
pub fn decode_query_ids(
    bytes: &CxxVector<u8>,
) -> Vec<u32> {
    let mut cursor = Cursor::new(bytes.as_slice());
    let (header, _) = read_file_header_and_flags(&mut cursor).unwrap();

    let mut query_ids: Vec<u32> = Vec::with_capacity(header.n_queries as usize);
    while let Ok((header, mut flags)) = read_block_header_and_flags(&mut cursor) {
        assert_eq!(header.num_records as usize, query_ids.len());
        assert_eq!(flags.query_ids.len(), query_ids.len());
        query_ids.append(&mut flags.query_ids);
        cursor.set_position(cursor.position() + header.block_len as u64);
    }

    query_ids
}
