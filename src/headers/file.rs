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

// File header for encoded data
//
// Always the first 32 bytes at the beginning of a .ahda v0.x file.
//
// Must always conform to this format.
//
#[derive(Clone, Encode, Decode, Default)]
pub struct FileHeader {
    /// Number of alignment targets.
    pub n_targets: u32,
    /// Number of query sequences (0 if unknown).
    pub n_queries: u32,
    /// Number of bytes in [FileFlags].
    pub flags_len: u32,
    /// Input format, see [Format](crate::Format) for details.
    pub format: u16,
    pub ph2: u16,
    pub ph3: u64,
    pub ph4: u64,
}

/// Data shared with all blocks
///
/// Variable length, use [FileHeader].flags_len to get size
///
/// Contents may differ between implementations.
///
#[derive(Clone, Encode, Decode)]
pub struct FileFlags {
    /// Query file basename
    pub query_name: String,
    /// Name and index of target sequences
    pub target_names: Vec<String>,
}

pub fn encode_file_header(
    n_targets: u32,
    n_queries: u32,
    flags_len: u32,
    format: u16,
    ph2: u16,
    ph3: u64,
    ph4: u64
) -> Result<Vec<u8>, E> {
    let mut bytes: Vec<u8> = Vec::new();
    let header_placeholder = FileHeader{ n_targets, n_queries, flags_len, format, ph2, ph3, ph4 };
    let nbytes = encode_into_std_write(
        &header_placeholder,
        &mut bytes,
        bincode::config::standard().with_fixed_int_encoding(),
    )?;
    assert_eq!(nbytes, 32);
    Ok(bytes)
}

pub fn decode_file_header(
    header_bytes: &[u8],
) -> Result<FileHeader, E> {
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
    conn.read_exact(&mut flags_bytes).unwrap();
    let res = decode_file_flags(&flags_bytes).unwrap();
    Ok(res)
}

pub fn encode_file_flags(
    flags: &FileFlags,
) -> Result<Vec<u8>, E> {
    let mut bytes: Vec<u8> = Vec::new();

    let _ = encode_into_std_write(
        flags,
        &mut bytes,
        bincode::config::standard(),
    )?;

    Ok(bytes)
}

pub fn decode_file_flags(
    bytes: &[u8],
) -> Result<FileFlags, E> {
    let flags = decode_from_slice(
        bytes,
        bincode::config::standard(),
    )?.0;

    Ok(flags)
}
