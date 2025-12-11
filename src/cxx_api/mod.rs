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

use crate::headers::file::encode_header_and_flags;
use crate::headers::file::build_header_and_flags;
use crate::encoder::bitmap_encoder::BitmapEncoder;
use crate::encoder::pack_roaring::pack_block;

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
