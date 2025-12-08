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
#[derive(Clone, Debug, Decode, Encode, PartialEq)]
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
#[derive(Clone, Debug, Decode, Encode, PartialEq)]
pub struct FileFlags {
    /// Query file basename
    pub query_name: String,
    /// Name and index of target sequences
    pub target_names: Vec<String>,
}

pub fn build_header_and_flags(
    targets: &[String],
    queries: &[String],
    sample: &str,
) -> Result<(FileHeader, FileFlags), E> {
    let flags = FileFlags{ target_names: targets.to_vec(), query_name: sample.to_string() };
    let flags_bytes = crate::headers::file::encode_file_flags(&flags).unwrap();
    let header = FileHeader{ n_targets: targets.len() as u32, n_queries: queries.len() as u32, flags_len: flags_bytes.len() as u32, format: 1_u16, ph2: 0, ph3: 0, ph4: 0 };
    Ok((header, flags))
}

pub fn encode_header_and_flags(
    header: &FileHeader,
    flags: &FileFlags,
) -> Result<Vec<u8>, E> {
    let mut bytes: Vec<u8> = encode_file_header(header)?;
    bytes.append(&mut encode_file_flags(flags)?);
    Ok(bytes)
}

pub fn encode_file_header(
    header: &FileHeader,
) -> Result<Vec<u8>, E> {
    let mut bytes: Vec<u8> = Vec::with_capacity(32);
    let nbytes = encode_into_std_write(
        &header,
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

pub fn read_file_header_and_flags<R: Read>(
    conn: &mut R,
) -> Result<(FileHeader, FileFlags), E> {
    let header = read_file_header(conn)?;
    let flags = read_file_flags(&header, conn)?;
    Ok((header, flags))
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
