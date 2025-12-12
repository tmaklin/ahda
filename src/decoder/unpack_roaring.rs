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
use crate::PseudoAln;
use crate::headers::block::BlockFlags;
use crate::headers::block::BlockHeader;
use crate::headers::block::decode_block_flags;
use crate::headers::file::FileFlags;

use std::collections::HashSet;
use std::io::Write;

use roaring::bitmap::RoaringBitmap;

use flate2::write::GzDecoder;

type E = Box<dyn std::error::Error>;

fn inflate_bytes(
    deflated: &[u8],
) -> Result<Vec<u8>, E> {
    let mut inflated: Vec<u8> = Vec::new();
    let mut decoder = GzDecoder::new(&mut inflated);
    decoder.write_all(deflated)?;
    decoder.finish()?;
    Ok(inflated)
}

pub fn unpack_block_roaring(
    bytes: &[u8],
    block_header: &BlockHeader,
) -> Result<(RoaringBitmap, BlockFlags), E> {
    let inflated_bytes = inflate_bytes(bytes)?;
    let inflated_bytes = inflate_bytes(&inflated_bytes)?;

    let block_flags = decode_block_flags(&inflated_bytes[(block_header.block_len as usize)..inflated_bytes.len()])?;

    let aln_bits = RoaringBitmap::deserialize_from(&inflated_bytes[0..(block_header.block_len as usize)])?;

    Ok((aln_bits, block_flags))
}
