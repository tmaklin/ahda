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

//! C++ bindings

use crate::decode_from_read_to_roaring;
use crate::headers::file::encode_header_and_flags;
use crate::headers::file::build_header_and_flags;
use crate::headers::file::read_file_header_and_flags;
use crate::headers::block::read_block_header_and_flags;
use crate::encoder::bitmap_encoder::BitmapEncoder;
use crate::encoder::pack_roaring::pack_block;

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

pub fn encode_block(
    queries: &CxxVector<CxxString>,
    query_ids: &CxxVector<u32>,
    set_bits: &CxxVector<u32>,
) -> Vec<u8> {
    let bitmap = RoaringBitmap::from_iter(set_bits.iter());
    let query_names: Vec<String> = queries.iter().map(|x| x.as_bytes().iter().map(|x| *x as char).collect::<String>()).collect();
    let block = pack_block(&query_names, query_ids.as_slice(), &bitmap);
    block.unwrap()
}

pub fn encode_bitmap(
    targets: &CxxVector<CxxString>,
    queries: &CxxVector<CxxString>,
    name: &CxxString,
    set_bits: &CxxVector<u32>,
) -> Vec<u8> {
    let query_names: Vec<String> = queries.iter().map(|x| x.as_bytes().iter().map(|x| *x as char).collect::<String>()).collect();
    let target_names: Vec<String> = targets.iter().map(|x| x.as_bytes().iter().map(|x| *x as char).collect::<String>()).collect();
    let query_name: String = name.as_bytes().iter().map(|x| *x as char).collect::<String>();

    let mut set_bits_iter = set_bits.as_slice().iter().cloned();
    let mut encoder = BitmapEncoder::new(&mut set_bits_iter, &target_names, &query_names, &query_name);

    let mut bytes: Vec<u8> = encoder.encode_header_and_flags().unwrap();
    for mut block in encoder.by_ref() {
        bytes.append(&mut block);
    }

    bytes
}

pub fn decode_bitmap(
    bytes: &CxxVector<u8>,
) -> Vec<u32> {
    let mut cursor = Cursor::new(bytes.as_slice());
    let (bitmap, _, _, _) = decode_from_read_to_roaring(&mut cursor).unwrap();
    let set_bits: Vec<u32> = bitmap.iter().collect();
    set_bits
}

pub fn decode_target_names(
    bytes: &CxxVector<u8>,
) -> Vec<String> {
    let mut cursor = Cursor::new(bytes.as_slice());
    let (header, flags) = read_file_header_and_flags(&mut cursor).unwrap();
    assert_eq!(header.n_targets as usize, flags.target_names.len());
    flags.target_names
}

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

    todo!("decoding block header and block flags")
}

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

    todo!("decoding block header and block flags")
}
