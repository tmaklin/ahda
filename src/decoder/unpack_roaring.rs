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
use crate::headers::file::FileFlags;

use std::collections::HashSet;
use std::io::Write;

use roaring::bitmap::RoaringBitmap;

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

pub fn decode_from_roaring(
    aln_bits: &RoaringBitmap,
    file_flags: &FileFlags,
    header: &BlockHeader,
    block_flags: &BlockFlags,
    n_targets: u32,
) -> Result<Vec<PseudoAln>, E> {
    let mut alns: Vec<PseudoAln> = Vec::with_capacity(header.num_records as usize);

    let mut prev_query_idx: Option<u32> = None;

    let mut ones: Vec<u32> = Vec::with_capacity(n_targets as usize);
    let mut query_idx: u32 = 0;

    let mut seen: HashSet<usize> = HashSet::with_capacity(header.num_records as usize);

    aln_bits.iter().for_each(|set_idx| {
        query_idx = set_idx / n_targets;
        if prev_query_idx.is_none() {
            prev_query_idx = Some(query_idx);
        }
        let target_idx = set_idx % n_targets;

        if prev_query_idx.unwrap() != query_idx {
            let name = block_flags.queries[*prev_query_idx.as_ref().unwrap() as usize].to_string();
            let id = block_flags.query_ids[*prev_query_idx.as_ref().unwrap() as usize];
            let ones_names: Vec<String> = ones.iter().map(|idx| file_flags.target_names[*idx as usize].clone()).collect();
            alns.push(PseudoAln{ ones_names: Some(ones_names), query_id: Some(id), ones: Some(ones.clone()), query_name: Some(name) });
            seen.insert(id as usize);
            ones.clear();

            ones.push(target_idx);
            prev_query_idx = Some(query_idx);
        } else {
            ones.push(target_idx);
        }
    });
    if prev_query_idx.is_some() {
        let name = block_flags.queries[query_idx as usize].to_string();
        let id = block_flags.query_ids[query_idx as usize];
        let ones_names: Vec<String> = ones.iter().map(|idx| file_flags.target_names[*idx as usize].clone()).collect();
        alns.push(PseudoAln{ ones_names: Some(ones_names), query_id: Some(id), ones: Some(ones.clone()), query_name: Some(name.to_string()) });
        seen.insert(id as usize);

        // Push results with no alignments
        block_flags.query_ids.iter().zip(block_flags.queries.iter()).for_each(|(idx, name)| {
            if !seen.contains(&(*idx as usize)) {
                alns.push(PseudoAln{ ones_names: Some(vec![]), query_id: Some(*idx), ones: Some(vec![]), query_name: Some(name.clone()) });
            }
        });
    }

    Ok(alns)
}

pub fn unpack_block_roaring(
    bytes: &[u8],
    block_header: &BlockHeader,
) -> Result<(RoaringBitmap, BlockFlags), E> {
    let inflated_bytes = inflate_bytes(bytes)?;
    let inflated_bytes = inflate_bytes(&inflated_bytes)?;

    let block_flags = decode_block_flags(&inflated_bytes[(block_header.block_len as usize)..inflated_bytes.len()])?;

    let aln_bits = RoaringBitmap::deserialize_from(&inflated_bytes[0..(block_header.block_len as usize)])?;

    Ok((aln_bits, block_flags))
}
