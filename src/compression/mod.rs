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

pub mod gzwrapper;
pub mod roaring32;
pub mod roaring64;

use crate::PseudoAln;
use crate::headers::file::FileHeader;

use roaring32::convert_to_roaring32;
use roaring32::pack_block_roaring32;
use roaring64::convert_to_roaring64;
use roaring64::pack_block_roaring64;

type E = Box<dyn std::error::Error>;

/// Supported bitmap types for an .ahda record
#[non_exhaustive]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum BitmapType {
    /// RoaringBitmap (32-bit address space)
    #[default]
    Roaring32,
    /// RoaringTreemap (64-bit address space)
    Roaring64,
}


impl BitmapType {
    pub fn from_u16(val: u16) -> Result<Self, E> {
        match val {
            0 => Ok(BitmapType::Roaring32),
            1 => Ok(BitmapType::Roaring64),
            _ => panic!("Not a valid BitmapType"),
        }
    }

    pub fn to_u16(&self) -> Result<u16, E> {
        match &self {
            BitmapType::Roaring32 => Ok(0),
            BitmapType::Roaring64 => Ok(1),
/// Supported compression methods for [FileFlags](crate::headers::file::FileFlags) and [BlockFlags](crate::headers::block::BlockFlags).
#[non_exhaustive]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum MetadataCompression {
    /// [bincode::config::standard]
    #[default]
    BincodeStandard,
    /// Gz with flate2
    Flate2,
}


impl MetadataCompression {
    pub fn from_u8(val: u8) -> Result<Self, E> {
        match val {
            0 => Ok(MetadataCompression::BincodeStandard),
            1 => Ok(MetadataCompression::Flate2),
            _ => panic!("Not a valid MetadataCompression"),
        }
    }

    pub fn to_u8(&self) -> u8 {
        match &self {
            MetadataCompression::BincodeStandard => 0,
            MetadataCompression::Flate2 => 1,
        }
    }
}

pub fn pack_records(
    file_header: &FileHeader,
    records: &[PseudoAln],
) -> Result<Vec<u8>, E> {
    let queries: Vec<String> = records.iter().filter_map(|record| {
        assert!(record.query_name.is_some());
        record.query_name.clone()
    }).collect();

    let query_ids: Vec<u32> = records.iter().filter_map(|record| {
        assert!(record.query_id.is_some());
        record.query_id
    }).collect();

    let block = match BitmapType::from_u16(file_header.bitmap_type)? {
        BitmapType::Roaring32 => {
            let bitmap = convert_to_roaring32(file_header, records)?;
            pack_block_roaring32(&queries, &query_ids, &bitmap)?
        },
        BitmapType::Roaring64 => {
            let bitmap = convert_to_roaring64(file_header, records)?;
            pack_block_roaring64(&queries, &query_ids, &bitmap)?
        }
    };

    Ok(block)
}
