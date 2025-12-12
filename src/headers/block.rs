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
use crate::compression::gzwrapper::deflate_bytes;

use std::io::Read;

use bincode::{Encode, Decode};
use bincode::encode_into_std_write;
use bincode::decode_from_slice;

type E = Box<dyn std::error::Error>;

// TODO Store information about what kind of bitmap is serialized in the block
//
// This could be used to change the bitmap implementation later without breaking
// backwards compatibility of the file format, or to optimize the storage
// further by using different compression schemes for inputs with different
// distributions.
//
#[derive(Clone, Debug, Decode, Encode, PartialEq)]
pub struct BlockHeader {
    pub num_records: u32,
    pub deflated_len: u32,
    pub block_len: u32,
    pub flags_len: u32,
    pub start_idx: u32,
    pub placeholder2: u32,
    pub placeholder3: u64,
}

/// Data about the records in this block
///
/// Variable length, use [BlockHeader].flags_len to get size
///
/// Contents may differ between implementations.
///
#[derive(Clone, Decode, Debug, Encode, PartialEq)]
pub struct BlockFlags {
    /// Names of query records
    pub queries: Vec<String>,
    pub query_ids: Vec<u32>,
}

pub fn encode_block_header(
    header: &BlockHeader,
) -> Result<Vec<u8>, E> {
    let mut bytes: Vec<u8> = Vec::new();
    let nbytes = encode_into_std_write(
        header,
        &mut bytes,
        bincode::config::standard().with_fixed_int_encoding(),
    )?;
    assert_eq!(nbytes, 32);
    Ok(bytes)
}

pub fn decode_block_header(
    header_bytes: &[u8],
) -> Result<BlockHeader, E> {
    Ok(decode_from_slice(header_bytes, bincode::config::standard().with_fixed_int_encoding())?.0)
}

pub fn read_block_header<R: Read>(
    conn: &mut R,
) -> Result<BlockHeader, E> {
    let mut header_bytes: [u8; 32] = [0_u8; 32];
    conn.read_exact(&mut header_bytes)?;
    let res = decode_block_header(&header_bytes)?;
    Ok(res)
}

pub fn read_block_flags<R: Read>(
    header: &BlockHeader,
    conn: &mut R,
) -> Result<BlockFlags, E> {
    let mut flags_bytes: Vec<u8> = vec![0; header.flags_len as usize];
    conn.read_exact(&mut flags_bytes).unwrap();
    let res = decode_block_flags(&flags_bytes).unwrap();
    Ok(res)
}

pub fn read_block_header_and_flags<R: Read>(
    conn: &mut R,
) -> Result<(BlockHeader, BlockFlags), E> {
    let header = read_block_header(conn)?;
    let flags = read_block_flags(&header, conn)?;
    Ok((header, flags))
}

pub fn encode_block_flags(
    flags: &BlockFlags,
) -> Result<Vec<u8>, E> {
    let mut bytes: Vec<u8> = Vec::new();

    let _ = encode_into_std_write(
        flags,
        &mut bytes,
        bincode::config::standard(),
    )?;

    let bytes = deflate_bytes(&bytes)?;
    Ok(bytes)
}

pub fn decode_block_flags(
    bytes: &[u8],
) -> Result<BlockFlags, E> {
    let flags: BlockFlags = decode_from_slice(bytes, bincode::config::standard())?.0;

    Ok(flags)
}

#[cfg(test)]
mod tests {

    #[test]
    fn encode_block_header() {
        use super::encode_block_header;
        use super::BlockHeader;

        let data = BlockHeader{ num_records: 31, deflated_len: 257, block_len: 65511, flags_len: 921, start_idx: 0, placeholder2: 0, placeholder3: 0 };
        let expected: Vec<u8> = vec![31, 0, 0, 0, 1, 1, 0, 0, 231, 255, 0, 0, 153, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];

        let got = encode_block_header(&data).unwrap();
        assert_eq!(got, expected);
    }

    #[test]
    fn decode_block_header() {
        use super::decode_block_header;
        use super::BlockHeader;

        let expected = BlockHeader{ num_records: 31, deflated_len: 257, block_len: 65511, flags_len: 921, start_idx: 0, placeholder2: 0, placeholder3: 0 };
        let data: Vec<u8> = vec![31, 0, 0, 0, 1, 1, 0, 0, 231, 255, 0, 0, 153, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];

        let got = decode_block_header(&data).unwrap();
        assert_eq!(got, expected);
    }

    #[test]
    fn read_block_header() {
        use super::read_block_header;
        use super::BlockHeader;

        use std::io::Cursor;

        let expected = BlockHeader{ num_records: 31, deflated_len: 257, block_len: 65511, flags_len: 921, start_idx: 0, placeholder2: 0, placeholder3: 0 };
        let data_bytes: Vec<u8> = vec![31, 0, 0, 0, 1, 1, 0, 0, 231, 255, 0, 0, 153, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let mut data: Cursor<Vec<u8>> = Cursor::new(data_bytes);

        let got = read_block_header(&mut data).unwrap();
        assert_eq!(got, expected);
    }

    #[test]
    fn encode_block_flags() {
        use super::encode_block_flags;
        use super::BlockFlags;

        let data = BlockFlags{ queries: vec!["a".to_string(), "b".to_string(), "c".to_string()], query_ids: vec![1, 0, 2] };
        let expected: Vec<u8> = vec![31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 99, 102, 76, 100, 76, 98, 76, 102, 102, 100, 96, 2, 0, 171, 14, 139, 110, 11, 0, 0, 0];

        let got = encode_block_flags(&data).unwrap();
        assert_eq!(got, expected);
    }

    #[test]
    fn decode_block_flags() {
        use super::decode_block_flags;
        use super::BlockFlags;

        let expected = BlockFlags{ queries: vec!["a".to_string(), "b".to_string(), "c".to_string()], query_ids: vec![1, 0, 2] };
        let data: Vec<u8> = vec![3, 1, 97, 1, 98, 1, 99, 3, 1, 0, 2];

        let got = decode_block_flags(&data).unwrap();
        assert_eq!(got, expected);
    }

    #[test]
    fn read_block_flags() {
        use super::read_block_flags;
        use super::BlockFlags;
        use super::BlockHeader;

        use std::io::Cursor;

        let expected = BlockFlags{ queries: vec!["a".to_string(), "b".to_string(), "c".to_string()], query_ids: vec![1, 0, 2] };
        let data_bytes: Vec<u8> = vec![3, 1, 97, 1, 98, 1, 99, 3, 1, 0, 2];
        let header = BlockHeader{ num_records: 31, deflated_len: 257, block_len: 65511, flags_len: data_bytes.len() as u32, start_idx: 0, placeholder2: 0, placeholder3: 0 };
        let mut data: Cursor<Vec<u8>> = Cursor::new(data_bytes);

        let got = read_block_flags(&header, &mut data).unwrap();
        assert_eq!(got, expected);
    }


    #[test]
    fn read_block_header_and_flags() {
        use super::read_block_header_and_flags;
        use super::BlockFlags;
        use super::BlockHeader;

        use std::io::Cursor;

        let data_bytes: Vec<u8> = vec![31, 0, 0, 0, 1, 1, 0, 0, 231, 255, 0, 0, 11, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 3, 1, 97, 1, 98, 1, 99, 3, 1, 0, 2];
        let mut data: Cursor<Vec<u8>> = Cursor::new(data_bytes);

        let expected_header = BlockHeader{ num_records: 31, deflated_len: 257, block_len: 65511, flags_len: 11 as u32, start_idx: 0, placeholder2: 0, placeholder3: 0 };
        let expected_flags = BlockFlags{ queries: vec!["a".to_string(), "b".to_string(), "c".to_string()], query_ids: vec![1, 0, 2] };

        let (got_header, got_flags) = read_block_header_and_flags(&mut data).unwrap();
        assert_eq!(got_header, expected_header);
        assert_eq!(got_flags, expected_flags);
    }
}
