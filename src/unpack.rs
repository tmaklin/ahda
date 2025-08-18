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
use crate::headers::block::decode_block_flags;

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
) -> Result<BVector, E> {
    let bits: BVector = bitmagic::BVector::deserialize(bytes)?;
    Ok(bits)
}

pub fn unpack<R: Read>(
    block_header: &BlockHeader,
    n_targets: usize,
    conn: &mut R,
) -> Result<Vec<PseudoAln>, E> {
    let mut deflated_bytes: Vec<u8> = vec![0; block_header.deflated_len as usize];
    conn.read_exact(&mut deflated_bytes)?;

    let inflated_bytes = inflate_bytes(&deflated_bytes)?;
    let inflated_bytes = inflate_bytes(&inflated_bytes)?;

    let aln_bytes = inflated_bytes[0..(block_header.block_len as usize)].to_vec();
    let flags_bytes = inflated_bytes[(block_header.block_len as usize)..inflated_bytes.len()].to_vec();

    let aln_bits = decode_bitvec(&aln_bytes)?;
    let block_flags = decode_block_flags(&flags_bytes)?;

    let mut alns: Vec<PseudoAln> = Vec::with_capacity(block_header.num_records as usize);
    let mut prev_query_idx = 0;
    let mut ones: Vec<u32> = Vec::with_capacity(n_targets);
    aln_bits.ones().for_each(|set_idx| {
        let query_idx = set_idx / n_targets;
        let target_idx = set_idx % n_targets;

        if prev_query_idx != query_idx {
            alns.push(PseudoAln{ ones_names: None, query_id: Some(query_idx as u32), ones: Some(ones.clone()), query_name: Some(block_flags.queries[prev_query_idx].clone()) });
            ones.clear();

            // Push results with no alignments
            for idx in (prev_query_idx + 1)..query_idx {
                alns.push(PseudoAln{ ones_names: None, query_id: Some(idx as u32), ones: Some(vec![]), query_name: Some(block_flags.queries[idx].clone()) });
            }

            ones.push(target_idx as u32);
            prev_query_idx = query_idx;
        } else {
            ones.push(target_idx as u32);
        }
    });

    Ok(alns)
}
