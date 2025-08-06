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
use crate::PseudoAln;
use crate::format::BlockHeader;

use std::io::Read;

use bitvec::{order::Lsb0, vec::BitVec};

use dsi_bitstream::traits::BE;
use dsi_bitstream::prelude::MemWordReader;
use dsi_bitstream::prelude::BufBitReader;
use dsi_bitstream::codes::RiceRead;
use dsi_bitstream::codes::MinimalBinaryRead;

type E = Box<dyn std::error::Error>;

#[derive(Debug, Clone)]
struct DecodeError {
    message: String,
}

impl std::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "ntx decoder error: {}", self.message)
    }
}

impl std::error::Error for DecodeError {}

fn minimal_binary_decode(
    encoded: &[u64],
    n_records: usize,
    param: u64,
) -> Result<Vec<u64>, E> {
    let mut reader = BufBitReader::<BE, _>::new(MemWordReader::new(encoded));

    let mut decoded: Vec<u64> = vec![0; n_records];
    for i in 0..n_records {
        decoded[i] = reader.read_minimal_binary(param)? - 1;
    }
    Ok(decoded)
}

pub fn decode_bitvec(
    bytes: &[u8],
    num_u64: usize,
    param: u64,
) -> Result<Vec<bool>, E> {
    let aln_u64: Vec<u64> = bytes.chunks(8).map(|chunk| {
        let mut arr: [u8; 8] = [0; 8];
        arr[0..chunk.len()].copy_from_slice(&chunk);
        u64::from_ne_bytes(arr)
    }).collect();

    let aln_decoded: Vec<u64> = minimal_binary_decode(&aln_u64, num_u64, param)?;

    let aln_flat = aln_decoded.iter().flat_map(|u64_rep| {
        let bits: BitVec<_, Lsb0> = BitVec::from_vec(u64_rep.to_ne_bytes().to_vec());
        let bits_vec: Vec<bool> = bits.iter().map(|bit| *bit).collect();
        bits_vec
    }).collect();

    Ok(aln_flat)
}

pub fn decode_ids(
    bytes: &[u8],
    num_ids: usize,
    param: u64,
) -> Result<Vec<u64>, E> {
    let ids_u64: Vec<u64> = bytes.chunks(8).map(|chunk| {
        let mut arr: [u8; 8] = [0; 8];
        arr[0..chunk.len()].copy_from_slice(&chunk);
        u64::from_ne_bytes(arr)
    }).collect();

    let ids = minimal_binary_decode(&ids_u64, num_ids, param)?;

    Ok(ids)
}
pub fn unpack<R: Read>(
    block_header: &BlockHeader,
    n_targets: usize,
    conn: &mut R,
) -> Result<Vec<PseudoAln>, E> {
    let mut id_bytes: Vec<u8> = Vec::with_capacity(block_header.ids_u64 as usize * 8_usize);
    conn.read_exact(&mut id_bytes)?;

    let mut aln_bytes: Vec<u8> = Vec::with_capacity(block_header.alignments_u64 as usize * 8_usize);
    conn.read_exact(&mut aln_bytes)?;

    let aln_bits = decode_bitvec(&aln_bytes, block_header.alignments_u64 as usize, block_header.alignments_param)?;
    let ids = decode_ids(&id_bytes, block_header.ids_u64 as usize, block_header.ids_param)?;

    let alns = ids.iter().enumerate().map(|(idx, id)| {
        PseudoAln{ read_id: *id as u32, ones: aln_bits[(idx * n_targets)..((idx + 1)*n_targets)].to_vec() }
    }).collect();

    Ok(alns)
}
