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

use dsi_bitstream::impls::MemWordWriterVec;
use dsi_bitstream::impls::BufBitWriter;
use dsi_bitstream::traits::BE;
use dsi_bitstream::codes::MinimalBinaryWrite;

type E = Box<dyn std::error::Error>;

#[derive(Debug, Clone)]
pub struct EncodeError;

impl std::fmt::Display for EncodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "invalid input to encode")
    }
}

impl std::error::Error for EncodeError {}

fn minimal_binary_encode(
    ints: &[u64],
) -> Result<(Vec<u64>, u64), E> {
    let param: u64 = *ints.iter().max().ok_or(EncodeError)?;
    assert!(param < u64::MAX - 2);
    let param = if param > u64::MAX - 2 { u64::MAX } else { param + 2 };

    let word_write = MemWordWriterVec::new(Vec::<u64>::new());
    let mut writer = BufBitWriter::<BE, _>::new(word_write);

    for n in ints {
        writer.write_minimal_binary(*n + 1, param)?;
    }

    writer.flush()?;

    Ok((writer.into_inner()?.into_inner(), param))
}

/// Converts [PseudoAln] records to BitMagic bitvectors
pub fn convert_to_bitmagic(
    records: &[PseudoAln],
) -> Result<(BVector, usize), E> {
    let offset = records.iter().map(|record| record.read_id).min().unwrap_or(0) as usize;

    let mut bits: BVector = BVector::new();

    let n_targets = records[0].ones.len();

    records.iter().enumerate().for_each(|(read_idx, record)| {
        record.ones.iter().enumerate().for_each(|(bit_idx, is_set)| {
            let index = read_idx*n_targets + bit_idx;
            bits.set(index, *is_set);
        });
    });

    Ok((bits, offset))
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
    let ids = records.iter().map(|record| {
        record.read_id as u64
    }).collect::<Vec<u64>>();
    let encoded_1 = minimal_binary_encode(&ids)?;

    let (alignments, offset) = convert_to_bitmagic(records)?;

    // TODO Error if offset * n_targets exceeds 32-bit BitMagic capacity
    let mut encoded_2 = serialize_bvector(&alignments)?;

    let mut data: Vec<u8> = encoded_1.0.iter().flat_map(|record| {
        let bytes: Vec<u8> = record.to_ne_bytes()[0..8].to_vec();
        let mut arr: [u8; 8] = [0; 8];
        arr[0..8].copy_from_slice(&bytes);
        arr
    }).collect();

    let mut block_flags: Vec<u8> = encode_block_flags(&Vec::new())?;
    let header = BlockHeader{ flags_len: block_flags.len() as u32,
                              num_records: records.len() as u32,
                              alignments_u64: encoded_2.len() as u32,
                              ids_u64: data.len() as u32,
                              alignments_param: offset as u64,
                              ids_param: encoded_1.1,
    };

    let mut block: Vec<u8> = encode_block_header(&header)?;
    assert_eq!(block.len(), 32);
    block.append(&mut block_flags);
    assert_eq!(block.len(), 32 + header.flags_len as usize);
    block.append(&mut data);
    assert_eq!(block.len(), 32 + header.flags_len as usize + header.ids_u64 as usize);
    block.append(&mut encoded_2);
    assert_eq!(block.len(), 32 + header.flags_len as usize + header.ids_u64 as usize + header.alignments_u64 as usize);

    Ok(block)
}
