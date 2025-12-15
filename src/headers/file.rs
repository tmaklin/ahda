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
use crate::AhdaVersion;
use crate::compression::BitmapType;
use crate::compression::MetadataCompression;

use std::io::Read;

use bincode::{Encode, Decode};
use bincode::encode_into_std_write;
use bincode::decode_from_slice;

type E = Box<dyn std::error::Error>;

// File header for encoded data
//
// Always the first 32 bytes at the beginning of a .ahda file.
//
// Must always conform to this format.
//
#[derive(Clone, Debug, Decode, Encode, PartialEq)]
pub struct FileHeader {
    /// Ahda header, consists of 32 ASCII bytes spelling "ahda".
    ///
    /// First four bytes can be used to check that a binary record is an ahda record.
    /// Next two bytes can be used to check which version of ahda was used to generate this file.
    pub ahda_header: [u8; 6], // = [97, 104, 100, 97, ...];

    /// File format version, indicates (in)compatible versions of the file format.
    pub file_format: u8,

    /// Compression method used for [FileFlags], see [MetadataCompression](crate::compression::MetadataCompression).
    pub metadata_compression: u8,

    /// Fields that must be present for records in this file, see [Format](crate::Format) for details.
    ///
    /// This indicates the fields of [PseudoAln] that must be decodable from all blocks.
    pub fields_present: u16,

    /// Number of alignment targets, this must match the length of `target_names` in [FileFlags].
    pub n_targets: u32,

    /// Number of query sequences, this must be greater or equal to the sum of all `num_records` in crate::headers::block::[BlockFlags].
    pub n_queries: u32,

    /// Bitmap type used to encode blocks in this file, see [BitmapType](crate::compression::BitmapType) for details.
    pub bitmap_type: u16,

    /// Block size (number of [PseudoAln] records) used to encode blocks in this file. Actual number of records per block may be different.
    pub block_size: u32,

    /// Number of bytes in [FileFlags] that follow the header bytes.
    pub flags_len: u64,
}

/// Data shared with all blocks
///
/// Variable length, use [FileHeader].flags_len to get size
///
/// Contents may differ between implementations.
///
#[non_exhaustive]
#[derive(Clone, Debug, Decode, Encode, PartialEq)]
pub struct FileFlags {
    /// Query file basename
    pub query_name: Option<String>,
    /// Name and index of target sequences
    pub target_names: Option<Vec<String>>,
}
pub fn build_ahda_header() -> [u8; 6] {
    const VERSION: &str = env!("CARGO_PKG_VERSION");
    let mut header = [97_u8, 104, 100, 97, 0, 0];
    let version: u16 = match VERSION {
        "0.1.0" => 0,
        _ => u16::MAX,
    };
    let version_bytes: [u8; 2] = version.to_le_bytes();
    header[4] = version_bytes[0];
    header[5] = version_bytes[1];
    header
}

#[derive(Debug, Clone)]
pub struct AhdaHeaderError;

impl std::fmt::Display for AhdaHeaderError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "bytes do not start with a valid ahda file header")
    }
}

impl std::error::Error for AhdaHeaderError {};

pub fn check_ahda_header(
    bytes: [u8; 6],
) -> Result<String, E> {
    let mut is_ahda = true;
    is_ahda &= bytes[0] == 97;
    is_ahda &= bytes[1] == 104;
    is_ahda &= bytes[2] == 100;
    is_ahda &= bytes[3] == 97;

    let version_bytes: [u8; 2] = [bytes[4], bytes[5]];
    let version: u16 = u16::from_le_bytes(version_bytes);
    let version_str: String = match version {
        0 => "0.1.0".to_string(),
        _ => "".to_string(),
    };

    if is_ahda {
        Ok(version_str)
    } else {
        Err(Box::new(AhdaHeaderError))
    }
}

pub fn build_file_header_and_flags(
    targets: &[String],
    n_queries: usize,
    query_name: &str,
    flags_compression: &MetadataCompression,
) -> Result<(FileHeader, FileFlags), E> {
    // Check if bitmap fits in 32-bit address space and adjust accordingly
    let n_targets = targets.len();
    let bitmap_size = (n_targets as u64) * (n_queries as u64);
    let bitmap_type = if bitmap_size < u32::MAX as u64 { BitmapType::Roaring32 } else { BitmapType::Roaring64 };

    // Adjust block size to fit within 32-bit address space if using RoaringBitmaps
    let block_size: u32 = match bitmap_type {
        BitmapType::Roaring32 => {
            let mut block_size = ((u32::MAX as u64) / (n_targets as u64)).min(65537_u64) as u32;
            block_size = if block_size == 1 { 2 } else { block_size - 1 };
            block_size
        },
        BitmapType::Roaring64 => {
            262144_u32
        },
    };

    let flags = FileFlags{ target_names: Some(targets.to_vec()), query_name: Some(query_name.to_string()) };
    let flags_bytes = encode_file_flags(&flags, &flags_compression).unwrap();

    let header = FileHeader{
        ahda_header: build_ahda_header(),
        file_format: AhdaVersion::V0_1_0.to_u8(),
        metadata_compression: flags_compression.to_u8(),
        fields_present: 0,
        n_targets: n_targets as u32,
        n_queries: n_queries as u32,
        bitmap_type: bitmap_type.to_u16(),
        block_size,
        flags_len: flags_bytes.len() as u64,
    };

    Ok((header, flags))
}

pub fn encode_file_header_and_flags(
    header: &mut FileHeader,
    flags: &FileFlags,
) -> Result<Vec<u8>, E> {
    // TODO set fields_present in header based on flags
    let mut flags_bytes: Vec<u8> = encode_file_flags(flags, &MetadataCompression::from_u8(header.metadata_compression)?)?;
    let flags_len: u64 = flags_bytes.len() as u64;
    header.flags_len = flags_len;

    let mut bytes: Vec<u8> = encode_file_header(header)?;
    bytes.append(&mut flags_bytes);
    Ok(bytes)
}

pub fn decode_file_header_and_flags(
    bytes: &[u8],
) -> Result<(FileHeader, FileFlags), E> {
    let header = decode_file_header(&bytes[0..32])?;
    assert!(bytes.len() >= 32 + header.flags_len as usize);
    let flags = decode_file_flags(&bytes[32..(header.flags_len as usize)], &MetadataCompression::from_u8(header.metadata_compression)?)?;
    Ok((header, flags))
}

pub fn encode_file_header(
    header: &FileHeader,
) -> Result<Vec<u8>, E> {
    let mut bytes: Vec<u8> = Vec::with_capacity(32);
    let nbytes = encode_into_std_write(
        header,
        &mut bytes,
        bincode::config::standard().with_fixed_int_encoding(),
    )?;
    assert_eq!(nbytes, 32);
    Ok(bytes)
}

pub fn decode_file_header(
    header_bytes: &[u8],
) -> Result<FileHeader, E> {
    assert_eq!(header_bytes.len(), 32);
    let mut bytes_start: [u8; 6] = [0; 6];
    bytes_start[0] = header_bytes[0];
    bytes_start[1] = header_bytes[1];
    bytes_start[2] = header_bytes[2];
    bytes_start[3] = header_bytes[3];
    bytes_start[4] = header_bytes[4];
    bytes_start[5] = header_bytes[5];
    let _ = check_ahda_header(bytes_start)?;
    Ok(decode_from_slice(header_bytes, bincode::config::standard().with_fixed_int_encoding())?.0)
}

pub fn read_file_header<R: Read>(
    conn: &mut R,
) -> Result<FileHeader, E> {
    let mut header_bytes: [u8; 32] = [0_u8; 32];
    conn.read_exact(&mut header_bytes)?;
    let res = decode_file_header(&header_bytes)?;
    Ok(res)
}

pub fn read_file_flags<R: Read>(
    header: &FileHeader,
    conn: &mut R,
) -> Result<FileFlags, E> {
    let mut flags_bytes: Vec<u8> = vec![0; header.flags_len as usize];
    conn.read_exact(&mut flags_bytes)?;
    let res = decode_file_flags(&flags_bytes, &MetadataCompression::from_u8(header.metadata_compression)?)?;
    Ok(res)
}

pub fn read_file_header_and_flags<R: Read>(
    conn: &mut R,
) -> Result<(FileHeader, FileFlags), E> {
    let header = read_file_header(conn)?;
    let flags = read_file_flags(&header, conn)?;
    Ok((header, flags))
}

pub fn encode_file_flags(
    flags: &FileFlags,
    compression: &MetadataCompression
) -> Result<Vec<u8>, E> {
    let mut bytes: Vec<u8> = Vec::new();

    match compression {
        MetadataCompression::BincodeStandard => {
            let _ = encode_into_std_write(
                flags,
                &mut bytes,
                bincode::config::standard(),
            )?;
        },
        MetadataCompression::Flate2 => {
            todo!("flate2 encoding for FileFlags")
        },
    }

    Ok(bytes)
}

pub fn decode_file_flags(
    bytes: &[u8],
    compression: &MetadataCompression
) -> Result<FileFlags, E> {
    let flags = match compression {
        MetadataCompression::BincodeStandard => {
            decode_from_slice(
                bytes,
                bincode::config::standard(),
            )?.0
        },
        MetadataCompression::Flate2 => {
            todo!("flate2 decoding for FileFlags")
        },
    };

    Ok(flags)
}

#[cfg(test)]
mod tests {

    #[test]
    fn build_header_and_flags() {
        use super::build_header_and_flags;
        use super::encode_file_flags;
        use super::FileHeader;
        use super::FileFlags;

        let targets = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let queries = vec!["1".to_string(), "2".to_string(), "3".to_string(), "4".to_string(), "5".to_string()];
        let sample = "sample";

        let expected_flags = FileFlags { query_name: sample.to_string(), target_names: targets.clone() };
        let nbytes = encode_file_flags(&expected_flags).unwrap().len();
        let expected_header = FileHeader { n_targets: targets.len() as u32, n_queries: queries.len() as u32, flags_len: nbytes as u64, format: 1_u16, bitmap_type: 0_u16, block_size: 0_u32, ph4: 0_u64 };

        let (got_header, got_flags) = build_header_and_flags(&targets, &queries, &sample).unwrap();

        assert_eq!(got_header, expected_header);
        assert_eq!(got_flags, expected_flags);
    }

    #[test]
    fn encode_header_and_flags() {
        use super::encode_header_and_flags;
        use super::encode_file_flags;
        use super::FileHeader;
        use super::FileFlags;

        let targets = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let queries = vec!["1".to_string(), "2".to_string(), "3".to_string(), "4".to_string(), "5".to_string()];
        let sample = "sample";

        let flags = FileFlags { query_name: sample.to_string(), target_names: targets.clone() };
        let nbytes = encode_file_flags(&flags).unwrap().len();
        let header = FileHeader { n_targets: targets.len() as u32, n_queries: queries.len() as u32, flags_len: nbytes as u64, format: 1_u16, bitmap_type: 0_u16, block_size: 0_u32, ph4: 0_u64 };

        let expected: Vec<u8> = vec![3, 0, 0, 0, 5, 0, 0, 0, 14, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 6, 115, 97, 109, 112, 108, 101, 3, 1, 97, 1, 98, 1, 99];

        let got = encode_header_and_flags(&header, &flags).unwrap();

        assert_eq!(got, expected);
    }

    #[test]
    fn encode_file_header() {
        use super::encode_file_header;
        use super::encode_file_flags;
        use super::FileHeader;
        use super::FileFlags;

        let targets = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let queries = vec!["1".to_string(), "2".to_string(), "3".to_string(), "4".to_string(), "5".to_string()];
        let sample = "sample";

        let flags = FileFlags { query_name: sample.to_string(), target_names: targets.clone() };
        let nbytes = encode_file_flags(&flags).unwrap().len();
        let header = FileHeader { n_targets: targets.len() as u32, n_queries: queries.len() as u32, flags_len: nbytes as u64, format: 1_u16, bitmap_type: 0_u16, block_size: 0_u32, ph4: 0_u64 };

        let expected: Vec<u8> = vec![3, 0, 0, 0, 5, 0, 0, 0, 14, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];

        let got = encode_file_header(&header).unwrap();

        assert_eq!(got, expected);
    }

    #[test]
    fn encode_file_flags() {
        use super::encode_file_flags;
        use super::FileFlags;

        let targets = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let sample = "sample";

        let flags = FileFlags { query_name: sample.to_string(), target_names: targets.clone() };

        let expected: Vec<u8> = vec![6, 115, 97, 109, 112, 108, 101, 3, 1, 97, 1, 98, 1, 99];

        let got = encode_file_flags(&flags).unwrap();

        assert_eq!(got, expected);
    }

    #[test]
    fn decode_file_header() {
        use super::decode_file_header;
        use super::FileHeader;

        let data: Vec<u8> = vec![3, 0, 0, 0, 5, 0, 0, 0, 14, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];

        let expected = FileHeader { n_targets: 3_u32, n_queries: 5_u32, flags_len: 14_u64, format: 1_u16, bitmap_type: 0_u16, block_size: 0_u32, ph4: 0_u64 };

        let got = decode_file_header(&data).unwrap();

        assert_eq!(got, expected);
    }

    #[test]
    fn decode_file_flags() {
        use super::decode_file_flags;
        use super::FileFlags;

        let targets = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let sample = "sample";

        let data: Vec<u8> = vec![6, 115, 97, 109, 112, 108, 101, 3, 1, 97, 1, 98, 1, 99];

        let expected = FileFlags { query_name: sample.to_string(), target_names: targets.clone() };

        let got = decode_file_flags(&data).unwrap();

        assert_eq!(got, expected);
    }

    #[test]
    fn read_file_header() {
        use super::read_file_header;
        use super::FileHeader;

        use std::io::Cursor;

        let data_bytes: Vec<u8> = vec![3, 0, 0, 0, 5, 0, 0, 0, 14, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 6, 115, 97, 109, 112, 108, 101, 3, 1, 97, 1, 98, 1, 99];
        let mut data: Cursor<Vec<u8>> = Cursor::new(data_bytes);

        let expected = FileHeader { n_targets: 3_u32, n_queries: 5_u32, flags_len: 14_u64, format: 1_u16, bitmap_type: 0_u16, block_size: 0_u32, ph4: 0_u64 };

        let got = read_file_header(&mut data).unwrap();

        assert_eq!(got, expected);
    }

    #[test]
    fn read_file_flags() {
        use super::read_file_flags;
        use super::FileHeader;
        use super::FileFlags;

        use std::io::Cursor;

        let targets = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let sample = "sample";

        let header = FileHeader { n_targets: 3_u32, n_queries: 5_u32, flags_len: 14_u64, format: 1_u16, bitmap_type: 0_u16, block_size: 0_u32, ph4: 0_u64 };
        let data_bytes: Vec<u8> = vec![6, 115, 97, 109, 112, 108, 101, 3, 1, 97, 1, 98, 1, 99, 3, 0, 0, 0, 5, 0, 0, 0, 14, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let mut data: Cursor<Vec<u8>> = Cursor::new(data_bytes);

        let expected = FileFlags { query_name: sample.to_string(), target_names: targets.clone() };

        let got = read_file_flags(&header, &mut data).unwrap();

        assert_eq!(got, expected);
    }

    #[test]
    fn read_file_header_and_flags() {
        use super::read_file_header_and_flags;
        use super::FileHeader;
        use super::FileFlags;

        use std::io::Cursor;

        let targets = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let queries = vec!["1".to_string(), "2".to_string(), "3".to_string(), "4".to_string(), "5".to_string()];
        let sample = "sample";

        let expected_flags = FileFlags { query_name: sample.to_string(), target_names: targets.clone() };
        let expected_header = FileHeader { n_targets: targets.len() as u32, n_queries: queries.len() as u32, flags_len: 14_u64, format: 1_u16, bitmap_type: 0_u16, block_size: 0_u32, ph4: 0_u64 };

        let data_bytes: Vec<u8> = vec![3, 0, 0, 0, 5, 0, 0, 0, 14, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 6, 115, 97, 109, 112, 108, 101, 3, 1, 97, 1, 98, 1, 99];
        let mut data: Cursor<Vec<u8>> = Cursor::new(data_bytes);

        let (got_header, got_flags) = read_file_header_and_flags(&mut data).unwrap();

        assert_eq!(got_header, expected_header);
        assert_eq!(got_flags, expected_flags);
    }
}
