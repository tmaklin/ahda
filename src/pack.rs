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
use crate::headers::block::encode_block_header;
use crate::headers::block::encode_block_flags;

use std::io::Write;

use bitmagic::BVector;

use flate2::write::GzEncoder;
use flate2::Compression;

type E = Box<dyn std::error::Error>;

#[derive(Debug, Clone)]
pub struct EncodeError;

impl std::fmt::Display for EncodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "invalid input to encode")
    }
}

impl std::error::Error for EncodeError {}

fn deflate_bytes(
    bytes: &[u8],
) -> Result<Vec<u8>, E> {
    let mut deflated: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut encoder = GzEncoder::new(&mut deflated, Compression::default());
    encoder.write_all(bytes)?;
    encoder.finish()?;
    Ok(deflated)
}

/// Converts [PseudoAln] records to BitMagic bitvectors
pub fn convert_to_bitmagic(
    records: &[PseudoAln],
    n_targets: usize,
) -> Result<BVector, E> {
    let mut bits: BVector = BVector::new();

    for record in records {
        if record.ones.is_none() || record.query_id.is_none() {
            return Err(Box::new(EncodeError{}))
        }
        let read_idx = record.query_id.unwrap() as usize;
        let ones = record.ones.as_ref().unwrap();
        ones.iter().for_each(|bit_idx| {
            let index = read_idx*n_targets + *bit_idx as usize;
            bits.set(index, true);
        });
    }

    Ok(bits)
}

pub fn serialize_bvector(
    bits: &BVector,
) -> Result<Vec<u8>, E> {
    let mut bytes: Vec<u8> = Vec::new();
    bits.serialize(&mut bytes)?;
    Ok(bytes)
}

pub fn pack(
    records: &[PseudoAln],
    n_targets: usize,
) -> Result<Vec<u8>, E> {
    let alignments = convert_to_bitmagic(records, n_targets)?;

    let serialized = serialize_bvector(&alignments)?;

    let query_names: Vec<String> = records.iter().filter_map(|record| {
        record.query_name.clone()
    }).collect();

    let mut block_flags: Vec<u8> = if !query_names.is_empty() {
        encode_block_flags(&query_names)?
    } else {
        encode_block_flags(&Vec::new())?
    };

    let flags_len = block_flags.len() as u32;
    let block_len = serialized.len() as u32;

    let mut flags_and_block: Vec<u8> = serialized;
    flags_and_block.append(&mut block_flags);

    // For some reason running twice is needed here?
    // Maybe related to window size somehow?
    //
    let deflated_1st = deflate_bytes(&flags_and_block)?;
    let mut deflated = deflate_bytes(&deflated_1st)?;

    let deflated_len = deflated.len() as u32;

    let header = BlockHeader{
        num_records: records.len() as u32,
        deflated_len,
        block_len,
        flags_len,
        placeholder1: 0,
        placeholder2: 0,
        placeholder3: 0,
    };

    let mut block: Vec<u8> = encode_block_header(&header)?;
    eprintln!("{:?}", header);
    block.append(&mut deflated);

    Ok(block)
}
