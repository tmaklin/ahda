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
use crate::headers::block::BlockFlags;
use crate::headers::block::BlockHeader;
use crate::headers::block::decode_block_flags;

use std::collections::HashSet;
use std::io::Read;
use std::io::Write;

use bitmagic::BVector;

use flate2::write::GzDecoder;

type E = Box<dyn std::error::Error>;

pub fn inflate_bytes(
    deflated: &[u8],
) -> Result<Vec<u8>, E> {
    let mut inflated: Vec<u8> = Vec::new();
    let mut decoder = GzDecoder::new(&mut inflated);
    decoder.write_all(deflated)?;
    decoder.finish()?;
    Ok(inflated)
}

pub fn decode_from_bitmagic(
    header: &BlockHeader,
    flags: &BlockFlags,
    n_targets: usize,
    bitmagic_bytes: &[u8],
) -> Result<Vec<PseudoAln>, E> {
    let aln_bits: BVector = bitmagic::BVector::deserialize(bitmagic_bytes)?;

    let mut alns: Vec<PseudoAln> = Vec::with_capacity(header.num_records as usize);

    let mut prev_query_idx: Option<usize> = None;

    let mut ones: Vec<u32> = Vec::with_capacity(n_targets);
    let mut query_idx: usize = 0;

    let mut seen: HashSet<usize> = HashSet::with_capacity(header.num_records as usize);

    aln_bits.ones().for_each(|set_idx| {
        query_idx = set_idx / n_targets;
        if prev_query_idx.is_none() {
            prev_query_idx = Some(query_idx);
        }
        let target_idx = set_idx % n_targets;

        if prev_query_idx.unwrap() != query_idx {
            let name = flags.queries[*prev_query_idx.as_ref().unwrap()].to_string();
            let id = flags.query_ids[*prev_query_idx.as_ref().unwrap()];
            alns.push(PseudoAln{ ones_names: None, query_id: Some(id), ones: Some(ones.clone()), query_name: Some(name) });
            seen.insert(id as usize);
            ones.clear();

            ones.push(target_idx as u32);
            prev_query_idx = Some(query_idx);
        } else {
            ones.push(target_idx as u32);
        }
    });
    if prev_query_idx.is_some() {
        let name = flags.queries[query_idx].to_string();
        let id = flags.query_ids[query_idx];
        alns.push(PseudoAln{ ones_names: None, query_id: Some(id), ones: Some(ones.clone()), query_name: Some(name.to_string()) });
        seen.insert(id as usize);

        // Push results with no alignments
        flags.query_ids.iter().zip(flags.queries.iter()).for_each(|(idx, name)| {
            if !seen.contains(&(*idx as usize)) {
                alns.push(PseudoAln{ ones_names: None, query_id: Some(*idx), ones: Some(vec![]), query_name: Some(name.clone()) });
            }
        });
    }

    Ok(alns)
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

    let block_flags = decode_block_flags(&inflated_bytes[(block_header.block_len as usize)..inflated_bytes.len()])?;

    let alns = decode_from_bitmagic(block_header, &block_flags, n_targets, &inflated_bytes[0..(block_header.block_len as usize)])?;

    Ok(alns)
}
