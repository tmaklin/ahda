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

use bitmagic::BVector;

type E = Box<dyn std::error::Error>;

#[derive(Debug, Clone)]
pub struct EncodeError;

impl std::fmt::Display for EncodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "invalid input to encode")
    }
}

impl std::error::Error for EncodeError {}

/// Converts [PseudoAln] records to BitMagic bitvectors
pub fn convert_to_bitmagic(
    records: &[PseudoAln],
) -> Result<BVector, E> {
    let mut bits: BVector = BVector::new();
    let n_targets = records[0].ones.len();

    records.iter().for_each(|record| {
        let read_idx = record.read_id as usize;
        record.ones.iter().enumerate().for_each(|(bit_idx, is_set)| {
            let index = read_idx*n_targets + bit_idx;
            bits.set(index, *is_set);
        });
    });

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
) -> Result<Vec<u8>, E> {
    let alignments = convert_to_bitmagic(records)?;

    let mut encoded_2 = serialize_bvector(&alignments)?;

    let mut block_flags: Vec<u8> = encode_block_flags(&Vec::new())?;
    let header = BlockHeader{ flags_len: block_flags.len() as u32,
                              num_records: records.len() as u32,
                              alignments_u64: encoded_2.len() as u32,
                              ids_u64: 0,
                              alignments_param: 0,
                              ids_param: 0,
    };

    let mut block: Vec<u8> = encode_block_header(&header)?;
    assert_eq!(block.len(), 32);
    block.append(&mut block_flags);
    assert_eq!(block.len(), 32 + header.flags_len as usize);
    block.append(&mut encoded_2);
    assert_eq!(block.len(), 32 + header.flags_len as usize + header.ids_u64 as usize + header.alignments_u64 as usize);

    Ok(block)
}
