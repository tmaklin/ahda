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

//! Wrappers for supported bitmap and metadata compression schemes.
//!
//! ## Bitmap compression schemes
//! Currently supported:
//! - Roaring bitmap
//! - Roaring treemap
//!
//! New schemes should implement functions to perform the following:
//! - Convert a [PseudoAln] array to the bitmap representation.
//! - Serialize the bitmap representation to bytes (u8).
//! - Deserialize bytes (u8) to the bitmap representation.
//! - Serialize the bitmap representation to a valid .ahda block record (u8 bytes).
//! - Deserialize a valid .ahda block record (u8 bytes) to the bitmap representation.
//!
//! ## Metadata compression schemes
//! Currently supported:
//! - Flate2
//!
//! New schemes should implement the following:
//! - Compress bytes (u8).
//! - Decompress bytes (u8).

pub mod gzwrapper;
pub mod roaringwrapper;

use roaring::RoaringBitmap;
use roaring::RoaringTreemap;

use crate::PseudoAln;
use crate::headers::file::FileHeader;

use roaringwrapper::convert_to_roaring;
use roaringwrapper::pack_block_roaring;

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
            _ => Err(Box::new(crate::errors::InvalidConversion { from: val.to_string(), to: "BitmapType".to_string() })),
        }
    }

    pub fn to_u16(&self) -> u16 {
        match &self {
            BitmapType::Roaring32 => 0,
            BitmapType::Roaring64 => 1,
        }
    }
}

/// Holder for supported [BitmapTypes](BitmapType)
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum BitmapHolder {
    Roaring32(RoaringBitmap),
    Roaring64(RoaringTreemap),
}

/// Supported compression methods for [FileFlags](crate::headers::file::FileFlags) and [BlockFlags](crate::headers::block::BlockFlags).
#[non_exhaustive]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum MetadataCompression {
    /// [bincode::config::standard]
    BincodeStandard,
    /// Gz with flate2
    #[default]
    Flate2,
}


impl MetadataCompression {
    pub fn from_u8(val: u8) -> Result<Self, E> {
        match val {
            0 => Ok(MetadataCompression::BincodeStandard),
            1 => Ok(MetadataCompression::Flate2),
            _ => Err(Box::new(crate::errors::InvalidConversion { from: val.to_string(), to: "MetadataCompression".to_string() })),
        }
    }

    pub fn to_u8(&self) -> u8 {
        match &self {
            MetadataCompression::BincodeStandard => 0,
            MetadataCompression::Flate2 => 1,
        }
    }
}

/// Compress a block of [PseudoAln] records.
pub fn pack_records(
    file_header: &FileHeader,
    records: Vec<PseudoAln>,
) -> Result<Vec<u8>, E> {
    let queries: Vec<Vec<u8>> = records.iter().filter_map(|record| {
        record.query_name.clone()
    }).collect();

    let query_names = if queries.is_empty() { None } else { Some(queries) };

    if query_names.is_none() && file_header.promises_query_names() {
        return Err(Box::new(crate::errors::HeaderPromiseNotHonoured { promise: "PseudoAln::query_name".to_string() }))
    }

    let query_ids: Vec<u32> = records.iter().filter_map(|record| {
        assert!(record.query_id.is_some());
        record.query_id
    }).collect();

    let bits = convert_to_roaring(file_header, records)?;
    let block = pack_block_roaring(file_header, &query_ids, query_names, bits)?;

    Ok(block)
}

#[cfg(test)]
mod tests {

    #[test]
    fn pack_records() {
        use super::pack_records;
        use crate::PseudoAln;
        use crate::AhdaFormatVersion;
        use crate::BitmapType;
        use crate::MetadataCompression;
        use crate::headers::file::FileHeader;
        use crate::headers::file::build_ahda_header;

        let data = vec![
            PseudoAln{ones_names: Some(vec!["chr.fasta".as_bytes().to_vec()]),  query_id: Some(1), ones: Some(vec![0]), query_name: Some("ERR4035126.2".as_bytes().to_vec()) },
            PseudoAln{ones_names: Some(vec!["chr.fasta".as_bytes().to_vec()]),  query_id: Some(0), ones: Some(vec![0]), query_name: Some("ERR4035126.1".as_bytes().to_vec()) },
            PseudoAln{ones_names: Some(vec!["chr.fasta".as_bytes().to_vec(), "plasmid.fasta".as_bytes().to_vec()]),  query_id: Some(2), ones: Some(vec![0, 1]), query_name: Some("ERR4035126.651903".as_bytes().to_vec()) },
            PseudoAln{ones_names: Some(vec![]),  query_id: Some(4), ones: Some(vec![]), query_name: Some("ERR4035126.16".as_bytes().to_vec()) },
            PseudoAln{ones_names: Some(vec!["plasmid.fasta".as_bytes().to_vec()]),  query_id: Some(3), ones: Some(vec![1]), query_name: Some("ERR4035126.7543".as_bytes().to_vec()) },
        ];

        let header = FileHeader {
            ahda_header: build_ahda_header().unwrap(),
            file_format: AhdaFormatVersion::V1_0_0.to_u8(),
            metadata_compression: MetadataCompression::Flate2.to_u8(),
            fields_present: crate::MASK_QUERY_IDS | crate::MASK_QUERIES,
            n_targets: 2_u32,
            n_queries: 5_u32,
            bitmap_type: BitmapType::Roaring32.to_u16(),
            block_size: 1000_u32,
            flags_len: 0_u64,
        };

        let expected = vec![5, 0, 0, 0, 0, 0, 0, 0, 40, 0, 0, 0, 64, 0, 0, 0, 0, 0, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 99, 100, 229, 113, 13, 10, 50, 49, 48, 54, 53, 52, 50, 211, 51, 66, 230, 24, 10, 34, 113, 204, 76, 13, 45, 13, 140, 121, 145, 165, 205, 248, 145, 120, 230, 166, 38, 198, 140, 172, 140, 12, 76, 44, 204, 0, 68, 178, 157, 37, 83, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 179, 50, 96, 96, 96, 100, 0, 1, 22, 6, 1, 48, 205, 196, 192, 194, 192, 202, 192, 206, 0, 0, 47, 109, 177, 38, 26, 0, 0, 0];
        let got = pack_records(&header, data).unwrap();

        assert_eq!(got, expected);

    }
}
