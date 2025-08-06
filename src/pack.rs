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
use crate::format::encode_block_header;

use std::io::Write;

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

pub fn pack(
    records: &[PseudoAln],
) -> Result<Vec<u8>, E> {
    let ids = records.iter().map(|record| {
        record.read_id as u64
    }).collect::<Vec<u64>>();
    let encoded_1 = minimal_binary_encode(&ids)?;

    let alignments = records.iter().flat_map(|record| {
        record.ones.chunks(64).map(|chunk| {
            let val = chunk.iter().rev().enumerate().fold(0, |acc, (i, b)| acc | (*b as u64) << i);
            val
        }).collect::<Vec<u64>>()
    }).collect::<Vec<u64>>();

    let encoded_2 = minimal_binary_encode(&alignments)?;

    let mut data: Vec<u8> = encoded_1.0.iter().chain(encoded_2.0.iter()).flat_map(|record| {
        let bytes: Vec<u8> = record.to_ne_bytes()[0..8].to_vec();
        let mut arr: [u8; 8] = [0; 8];
        arr[0..8].copy_from_slice(&bytes);
        arr
    }).collect();

    let header = BlockHeader{ block_size: 256 + data.len() as u32,
                              num_records: records.len() as u32,
                              alignments_u64: encoded_2.0.len() as u32,
                              ids_u64: encoded_1.0.len() as u32,
                              alignments_param: encoded_2.1,
                              ids_param: encoded_1.1,
    };

    let mut block: Vec<u8> = encode_block_header(&header)?;
    block.append(&mut data);

    Ok(block)
}
