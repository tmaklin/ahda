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
use crate::headers::block::BlockHeader;

use std::io::Read;

use bitmagic::BVector;

use dsi_bitstream::traits::BE;
use dsi_bitstream::prelude::MemWordReader;
use dsi_bitstream::prelude::BufBitReader;
use dsi_bitstream::codes::MinimalBinaryRead;

type E = Box<dyn std::error::Error>;

fn minimal_binary_decode(
    encoded: &[u64],
    n_records: usize,
    param: u64,
) -> Result<Vec<u64>, E> {
    let mut reader = BufBitReader::<BE, _>::new(MemWordReader::new(encoded));

    let mut decoded: Vec<u64> = vec![0; n_records];
    for item in decoded.iter_mut().take(n_records) {
        *item = reader.read_minimal_binary(param)? - 1;
    }
    Ok(decoded)
}

pub fn decode_bitvec(
    bytes: &[u8],
    num_records: usize,
    n_targets: usize,
) -> Result<Vec<bool>, E> {
    let bits: BVector = bitmagic::BVector::deserialize(bytes)?;

    assert!(num_records * n_targets > num_records.max(n_targets)); // check for overflow

    let mut res: Vec<bool> = vec![false; num_records * n_targets];
    bits.ones().for_each(|idx| res[idx] = true);

    Ok(res)
}

pub fn decode_ids(
    bytes: &[u8],
    num_ids: usize,
    param: u64,
) -> Result<Vec<u64>, E> {
    let ids_u64: Vec<u64> = bytes.chunks(8).map(|chunk| {
        let mut arr: [u8; 8] = [0; 8];
        arr[0..chunk.len()].copy_from_slice(chunk);
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
    let mut id_bytes: Vec<u8> = vec![0; block_header.ids_u64 as usize];
    conn.read_exact(&mut id_bytes)?;

    let mut aln_bytes: Vec<u8> = vec![0; block_header.alignments_u64 as usize];
    conn.read_exact(&mut aln_bytes)?;

    let aln_bits = decode_bitvec(&aln_bytes, block_header.num_records as usize, n_targets)?;

    let ids = decode_ids(&id_bytes, block_header.num_records as usize, block_header.ids_param)?;

    assert_eq!(ids.len(), block_header.num_records as usize);
    assert_eq!(aln_bits.len() / n_targets, block_header.num_records as usize);

    let alns = ids.iter().enumerate().map(|(idx, id)| {
        let start: usize = idx  * n_targets;
        let end: usize = (idx + 1) * n_targets;
        PseudoAln{ read_id: *id as u32, ones: aln_bits[start..end].to_vec() }
    }).collect();

    Ok(alns)
}
