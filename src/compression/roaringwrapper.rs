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

//! Roaring treemap wrapper (64-bit address space).

use crate::PseudoAln;
use crate::headers::block::BlockFlags;
use crate::headers::block::BlockHeader;
use crate::headers::file::FileHeader;
use crate::headers::block::encode_block_header;
use crate::headers::block::encode_block_flags;
use crate::headers::block::decode_block_flags;

use crate::compression::gzwrapper::deflate_bytes;
use crate::compression::gzwrapper::inflate_bytes;

use super::BitmapType;
use super::BitmapHolder;
use super::MetadataCompression;

use roaring::bitmap::RoaringBitmap;
use roaring::treemap::RoaringTreemap;

type E = Box<dyn std::error::Error>;

/// Converts [PseudoAln] records to a BitmapHolder holding the bitmap type specified in [FileHeader]
pub fn convert_to_roaring(
    file_header: &FileHeader,
    records: Vec<PseudoAln>,
) -> Result<BitmapHolder, E> {
    let n_targets: u64 = file_header.n_targets as u64;

    let mut bits = match BitmapType::from_u16(file_header.bitmap_type)? {
        BitmapType::Roaring32 => BitmapHolder::Roaring32(RoaringBitmap::new()),
        BitmapType::Roaring64 => BitmapHolder::Roaring64(RoaringTreemap::new()),
    };

    for record in records.iter() {
        if record.ones.is_none() || record.query_id.is_none() {
            return Err(Box::new(crate::errors::EncodeError{}))
        }
        let ones = record.ones.as_ref().unwrap();
        let idx = *record.query_id.as_ref().unwrap() as u64;
        match &mut bits {
            BitmapHolder::Roaring32(bits) => {
                ones.iter().for_each(|bit_idx| {
                    let index = idx as u32 *n_targets as u32 + *bit_idx;
                    bits.insert(index);
                });
            },
            BitmapHolder::Roaring64(bits) => {
                ones.iter().map(|x| *x as u64).for_each(|bit_idx| {
                    let index = idx * n_targets + bit_idx;
                    bits.insert(index);
                });
            }
        }
    }

    Ok(bits)
}

pub fn serialize_roaring(
    bits: BitmapHolder,
) -> Result<Vec<u8>, E> {
    let mut bytes: Vec<u8> = Vec::new();
    match bits {
        BitmapHolder::Roaring32(bits) => bits.serialize_into(&mut bytes)?,
        BitmapHolder::Roaring64(bits) => bits.serialize_into(&mut bytes)?,
    }
    let bytes = deflate_bytes(&bytes)?;
    Ok(bytes)
}

pub fn deserialize_roaring32(
    bytes: &[u8],
) -> Result<RoaringBitmap, E> {
    let bitmap_bytes = inflate_bytes(bytes)?;
    let bitmap = RoaringBitmap::deserialize_from(bitmap_bytes.as_slice())?;
    Ok(bitmap)
}

pub fn deserialize_roaring64(
    bytes: &[u8],
) -> Result<RoaringTreemap, E> {
    let bitmap_bytes = inflate_bytes(bytes)?;
    let bitmap = RoaringTreemap::deserialize_from(bitmap_bytes.as_slice())?;
    Ok(bitmap)
}

pub fn pack_block_roaring(
    queries: &[Vec<u8>],
    query_ids: &[u32],
    bitmap: BitmapHolder,
) -> Result<Vec<u8>, E> {
    let bitmap_type = match bitmap {
        BitmapHolder::Roaring32(_) => BitmapType::Roaring32,
        BitmapHolder::Roaring64(_) => BitmapType::Roaring64,
    };

    let mut serialized = serialize_roaring(bitmap)?;

    let flags: BlockFlags = BlockFlags{ queries: Some(queries.to_vec()), query_ids: Some(query_ids.to_vec()) };
    let fields_present = flags.fields_present();
    let mut block_flags: Vec<u8> = encode_block_flags(&flags)?;

    let flags_len = block_flags.len() as u64;
    let block_len = serialized.len() as u32;


    let header = BlockHeader{
        num_records: queries.len() as u32,
        block_len,
        flags_len,
        bitmap_type: bitmap_type.to_u16(),
        metadata_compression: MetadataCompression::default().to_u8(),
        fields_present,
        placeholder1: 0,
        placeholder2: 0,
        placeholder3: 0,
    };

    let mut block: Vec<u8> = encode_block_header(&header)?;
    block.append(&mut block_flags);
    block.append(&mut serialized);

    Ok(block)
}

pub fn unpack_block_roaring(
    bytes: &[u8],
    block_header: &BlockHeader,
) -> Result<(BitmapHolder, BlockFlags), E> {
    let block_flags = decode_block_flags(&bytes[0..(block_header.flags_len as usize)])?;
    let bitmap = match BitmapType::from_u16(block_header.bitmap_type)? {
        BitmapType::Roaring32 => BitmapHolder::Roaring32(deserialize_roaring32(&bytes[(block_header.flags_len as usize)..((block_header.flags_len + block_header.block_len as u64).try_into()?)])?),
        BitmapType::Roaring64 => BitmapHolder::Roaring64(deserialize_roaring64(&bytes[(block_header.flags_len as usize)..((block_header.flags_len + block_header.block_len as u64).try_into()?)])?),
    };

    Ok((bitmap, block_flags))
}
