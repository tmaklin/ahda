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
use std::io::Write;

use bitmagic::BVector;

use flate2::write::GzDecoder;

type E = Box<dyn std::error::Error>;

fn inflate_bytes(
    deflated: &[u8],
) -> Result<Vec<u8>, E> {
    let mut inflated: Vec<u8> = Vec::new();
    let mut decoder = GzDecoder::new(&mut inflated);
    decoder.write_all(deflated)?;
    decoder.finish()?;
    Ok(inflated)
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

pub fn unpack<R: Read>(
    block_header: &BlockHeader,
    n_targets: usize,
    conn: &mut R,
) -> Result<Vec<PseudoAln>, E> {
    let mut deflated_bytes: Vec<u8> = vec![0; block_header.alignments_u64 as usize];
    conn.read_exact(&mut deflated_bytes)?;

    let aln_bytes = inflate_bytes(&deflated_bytes)?;
    let aln_bits = decode_bitvec(&aln_bytes, block_header.num_records as usize, n_targets)?;

    assert_eq!(aln_bits.len() / n_targets, block_header.num_records as usize);

    let alns = aln_bits.chunks(n_targets).enumerate().map(|(idx, _)| {
        let start: usize = idx  * n_targets;
        let end: usize = (idx + 1) * n_targets;
        let ones: Vec<u32> = aln_bits[start..end].iter().enumerate().filter_map(|(idx, is_set)| if *is_set { Some(idx as u32) } else { None }).collect();
        PseudoAln{ query_id: Some(idx as u32), ones, ..Default::default()}
    }).collect();

    Ok(alns)
}
