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
use std::io::Read;

use bincode::{Encode, Decode};
use bincode::encode_into_std_write;
use bincode::decode_from_slice;

type E = Box<dyn std::error::Error>;

#[derive(Encode, Debug, Decode)]
pub struct BlockHeader {
    pub num_records: u32,
    pub deflated_len: u32,
    pub block_len: u32,
    pub flags_len: u32,
    pub placeholder1: u32,
    pub placeholder2: u32,
    pub placeholder3: u64,
}

/// Data about the records in this block
///
/// Variable length, use [BlockHeader].flags_len to get size
///
/// Contents may differ between implementations.
///
#[derive(Encode, Decode)]
pub struct BlockFlags {
    /// Names of query records
    pub queries: Vec<String>,
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

pub fn encode_block_flags(
    queries: &[String],
) -> Result<Vec<u8>, E> {
    let mut bytes: Vec<u8> = Vec::new();

    let _ = encode_into_std_write(
        queries,
        &mut bytes,
        bincode::config::standard(),
    )?;

    Ok(bytes)
}

pub fn decode_block_flags(
    bytes: &[u8],
) -> Result<Vec<String>, E> {
    let queries: Vec<String> = decode_from_slice(
        bytes,
        bincode::config::standard(),
    )?.0;

    Ok(queries)
}
