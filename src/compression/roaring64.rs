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
use crate::headers::file::FileHeader;
use crate::headers::block::encode_block_header;
use crate::headers::block::encode_block_flags;
use crate::headers::block::decode_block_flags;

use crate::compression::gzwrapper::deflate_bytes;
use crate::compression::gzwrapper::inflate_bytes;

use roaring::treemap::RoaringTreemap;

type E = Box<dyn std::error::Error>;

#[derive(Debug, Clone)]
pub struct EncodeError;

impl std::fmt::Display for EncodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "invalid input to encode")
    }
}

impl std::error::Error for EncodeError {}

/// Converts [PseudoAln] records to RoaringTreemap
pub fn convert_to_roaring64(
    file_header: &FileHeader,
    records: &[PseudoAln],
) -> Result<RoaringTreemap, E> {
    let n_targets: usize = file_header.n_targets as usize;
    let mut bits: RoaringTreemap = RoaringTreemap::new();

    for record in records.iter() {
        if record.ones.is_none() || record.query_id.is_none() {
            return Err(Box::new(EncodeError{}))
        }
        let ones = record.ones.as_ref().unwrap();
        let idx = *record.query_id.as_ref().unwrap();
        ones.iter().for_each(|bit_idx| {
            let index = idx as u64 * n_targets as u64 + *bit_idx as u64;
            bits.insert(index);
        });
    }

    Ok(bits)
}

pub fn serialize_roaring64(
    bits: &RoaringTreemap,
) -> Result<Vec<u8>, E> {
    let mut bytes: Vec<u8> = Vec::new();
    bits.serialize_into(&mut bytes)?;
    let bytes = deflate_bytes(&bytes)?;
    Ok(bytes)
}

pub fn deserialize_roaring64(
    bytes: &[u8],
) -> Result<RoaringTreemap, E> {
    let bitmap_bytes = inflate_bytes(bytes)?;
    let bitmap = RoaringTreemap::deserialize_from(bitmap_bytes.as_slice())?;
    Ok(bitmap)
}

pub fn pack_block_roaring64(
    queries: &[String],
    query_ids: &[u32],
    bitmap: &RoaringTreemap,
) -> Result<Vec<u8>, E> {
    let mut serialized = serialize_roaring64(bitmap)?;

    let flags: BlockFlags = BlockFlags{ queries: queries.to_vec(), query_ids: query_ids.to_vec() };
    let mut block_flags: Vec<u8> = encode_block_flags(&flags)?;

    let flags_len = block_flags.len() as u32;
    let block_len = serialized.len() as u32;

    let deflated_len = flags_len + block_len;

    let header = BlockHeader{
        num_records: queries.len() as u32,
        deflated_len,
        block_len,
        flags_len,
        start_idx: *query_ids.iter().min().unwrap(),
        placeholder2: 0,
        placeholder3: 0,
    };

    let mut block: Vec<u8> = encode_block_header(&header)?;
    block.append(&mut block_flags);
    block.append(&mut serialized);

    Ok(block)
}

pub fn unpack_block_roaring64(
    bytes: &[u8],
    block_header: &BlockHeader,
) -> Result<(RoaringTreemap, BlockFlags), E> {
    let block_flags = decode_block_flags(&bytes[0..(block_header.flags_len as usize)])?;
    let bitmap = deserialize_roaring64(&bytes[(block_header.flags_len as usize)..((block_header.flags_len + block_header.block_len) as usize)])?;
    Ok((bitmap, block_flags))
}
