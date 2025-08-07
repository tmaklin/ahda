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

#[derive(Encode, Decode)]
pub struct FileHeader {
    pub n_targets: u64,
    pub ph2: u64,
    pub ph3: u64,
    pub ph4: u64,
}

pub fn encode_file_header(
    n_targets: u64,
    ph2: u64,
    ph3: u64,
    ph4: u64
) -> Result<Vec<u8>, E> {
    let mut bytes: Vec<u8> = Vec::new();
    let header_placeholder = FileHeader{ n_targets, ph2, ph3, ph4 };
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
