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
//!
//! Currently supported:
//! - Roaring bitmap
//! - Roaring treemap
//!
//! ### Adding new bitmap compression schemes
//!
//! New schemes should implement functions to perform the following:
//! - Convert a [PseudoAln] array to the bitmap representation.
//! - Compress a bitmap into a valid ahda block.
//! - Decompress an u8 array into a bitmap and [BlockFlags](crate::headers::block::BlockFlags).
//! - Add the scheme to the [BitmapType] and [BitmapHolder] enums and handle it where required.
//!
//! See the [Roaring wrapper](roaringwrapper) for examples.
//!
//! ## Metadata compression schemes
//!
//! Currently supported:
//! - Flate2
//! - Bincode (standard encoding with no compression)
//!
//! ### Adding new metadata compression schemes
//!
//! New schemes should implement functions that do the following:
//! - Compress an u8 array and return an u8 vector.
//! - Decompress an u8 array and return an u8 vector.
//! - Add the scheme to the [MetadataCompression] enum and handle it where required.
//!
//! See the [Flate2 wrapper](gzwrapper) for examples.
//!

pub mod gzwrapper;
pub mod roaringwrapper;

use roaring::RoaringBitmap;
use roaring::RoaringTreemap;

use crate::PseudoAln;
use crate::headers::file::FileHeader;

use crate::errors::HeaderPromiseNotHonoured;
use crate::errors::InvalidConversion;

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
    /// Convert the u16 value stored in [FileHeader] or [BlockHeader](crate::headers::block::BlockHeader) into a valid BitmapType.
    ///
    /// ## Errors
    ///
    /// Returns [InvalidConversion] if the u16 value does not correspond to a valid BitmapType.
    ///
    /// ## Usage
    ///
    /// ```rust
    /// use ahda::compression::BitmapType;
    /// let u16_rep = 0_u16;
    /// let bitmap_type = BitmapType::from_u16(u16_rep).unwrap();
    /// assert_eq!(bitmap_type, BitmapType::Roaring32);
    /// ```
    pub fn from_u16(val: u16) -> Result<Self, E> {
        match val {
            0 => Ok(BitmapType::Roaring32),
            1 => Ok(BitmapType::Roaring64),
            _ => Err(Box::new(InvalidConversion { from: val.to_string(), to: "BitmapType".to_string() })),
        }
    }

    /// Convert a BitmapType into a u16 value for storage in [FileHeader] or [BlockHeader](crate::headers::block::BlockHeader).
    ///
    /// ## Usage
    ///
    /// ```rust
    /// use ahda::compression::BitmapType;
    /// let bitmap_type = BitmapType::Roaring32;
    /// let u16_rep = bitmap_type.to_u16();
    /// assert_eq!(u16_rep, 0_u16);
    /// ```
    pub fn to_u16(&self) -> u16 {
        match &self {
            BitmapType::Roaring32 => 0,
            BitmapType::Roaring64 => 1,
        }
    }
}

/// Holder for supported [BitmapTypes](BitmapType).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum BitmapHolder {
    /// [RoaringBitmap] (32-bit address space).
    Roaring32(RoaringBitmap),
    /// [RoaringTreemap] (64-bit address space).
    Roaring64(RoaringTreemap),
}

/// Supported compression methods for [FileFlags](crate::headers::file::FileFlags) and [BlockFlags](crate::headers::block::BlockFlags).
#[non_exhaustive]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum MetadataCompression {
    /// [Bincode](bincode::config::standard) without compression.
    BincodeStandard,
    /// Gz compression with flate2.
    #[default]
    Flate2,
}


impl MetadataCompression {
    /// Convert the u8 value stored in [FileHeader] or [BlockHeader](crate::headers::block::BlockHeader) into a valid MetadataCompression.
    ///
    /// ## Errors
    ///
    /// Returns [InvalidConversion] if the u8 value does not correspond to a valid MetadataCompression.
    ///
    /// ## Usage
    ///
    /// ```rust
    /// use ahda::compression::MetadataCompression;
    /// let u8_rep = 1_u8;
    /// let compression_type = MetadataCompression::from_u8(u8_rep).unwrap();
    /// assert_eq!(compression_type, MetadataCompression::Flate2);
    /// ```
    pub fn from_u8(val: u8) -> Result<Self, E> {
        match val {
            0 => Ok(MetadataCompression::BincodeStandard),
            1 => Ok(MetadataCompression::Flate2),
            _ => Err(Box::new(InvalidConversion { from: val.to_string(), to: "MetadataCompression".to_string() })),
        }
    }

    /// Convert a MetadataCompression into a u8 value for storage in [FileHeader] or [BlockHeader](crate::headers::block::BlockHeader).
    ///
    /// ## Usage
    ///
    /// ```rust
    /// use ahda::compression::MetadataCompression;
    /// let compression_type = MetadataCompression::Flate2;
    /// let u8_rep = compression_type.to_u8();
    /// assert_eq!(u8_rep, 1_u8);
    /// ```
    pub fn to_u8(&self) -> u8 {
        match &self {
            MetadataCompression::BincodeStandard => 0,
            MetadataCompression::Flate2 => 1,
        }
    }
}

/// Compress a block of [PseudoAln] records using the compression types specified in the [FileHeader].
///
/// **Note**: this function assumes that the FileHeader is correctly specified for
/// the input data. If you want to compress records without specifying the
/// header manually, use [Encoder](crate::encoder::Encoder).
///
/// ## Errors
///
/// Returns [HeaderPromiseNotHonoured] if the FileHeader requires that a field
/// in the [PseudoAln] records is filled but it was not.
///
/// ## Panics
///
/// Will panic if
/// - Query names are filled for some records but not all.
///
/// ## Usage
///
/// ```rust
/// use ahda::PseudoAln;
/// use ahda::headers::file::FileHeader;
/// use ahda::compression::pack_records;
///
/// let data = vec![
///     PseudoAln{ones_names: Some(vec!["target_1".as_bytes().to_vec()]),  query_id: Some(1), ones: Some(vec![0]), query_name: Some("query_2".as_bytes().to_vec()) },
///     PseudoAln{ones_names: Some(vec![]),  query_id: Some(0), ones: Some(vec![]), query_name: Some("query_1".as_bytes().to_vec()) },
///     PseudoAln{ones_names: Some(vec!["target_1".as_bytes().to_vec(), "target_7".as_bytes().to_vec()]),  query_id: Some(17), ones: Some(vec![0, 6]), query_name: Some("query_16".as_bytes().to_vec()) },
/// ];
///
/// let mut header = FileHeader::default();
/// header.n_queries = 20;
/// header.n_targets = 10;
///
/// let expected: Vec<u8> = vec![3, 0, 0, 0, 1, 0, 0, 0, 39, 0, 0, 0, 41, 0, 0, 0, 0, 0, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 99, 100, 102, 47, 44, 77, 45, 170, 140, 55, 130, 210, 134, 28, 80, 218, 140, 145, 153, 145, 65, 16, 0, 133, 237, 180, 205, 32, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 179, 50, 96, 96, 96, 100, 0, 1, 38, 6, 1, 6, 6, 6, 46, 134, 85, 12, 27, 24, 0, 255, 108, 49, 129, 22, 0, 0, 0];
/// let got = pack_records(&header, data).unwrap();
///
/// assert_eq!(got, expected);
/// ```
pub fn pack_records(
    file_header: &FileHeader,
    records: Vec<PseudoAln>,
) -> Result<Vec<u8>, E> {
    let queries: Vec<Vec<u8>> = records.iter().filter_map(|record| {
        record.query_name.clone()
    }).collect();

    let query_names = if queries.is_empty() {
        None
    } else {
        assert_eq!(queries.len(), records.len());
        Some(queries)
    };

    if query_names.is_none() && file_header.promises_query_names() {
        return Err(Box::new(HeaderPromiseNotHonoured { promise: "PseudoAln::query_name".to_string() }))
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

        let expected = vec![5, 0, 0, 0, 1, 0, 0, 0, 40, 0, 0, 0, 64, 0, 0, 0, 0, 0, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 99, 100, 229, 113, 13, 10, 50, 49, 48, 54, 53, 52, 50, 211, 51, 66, 230, 24, 10, 34, 113, 204, 76, 13, 45, 13, 140, 121, 145, 165, 205, 248, 145, 120, 230, 166, 38, 198, 140, 172, 140, 12, 76, 44, 204, 0, 68, 178, 157, 37, 83, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 179, 50, 96, 96, 96, 100, 0, 1, 22, 6, 1, 48, 205, 196, 192, 194, 192, 202, 192, 206, 0, 0, 47, 109, 177, 38, 26, 0, 0, 0];
        let got = pack_records(&header, data).unwrap();

        assert_eq!(got, expected);

    }
}
