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
use crate::Format;
use crate::PseudoAln;
use crate::headers::file::FileHeader;
use crate::headers::file::FileFlags;
use crate::headers::file::encode_file_header;
use crate::headers::file::encode_file_flags;

use std::collections::HashMap;

// TODO records should be anything that implements `next`
pub struct Encoder<'a, I: Iterator> where I: Iterator<Item=PseudoAln> {
    // Inputs
    records: &'a mut I,
    query_to_pos: &'a HashMap<String, usize>,
    pos_to_query: &'a HashMap<usize, String>,

    // These are given as construtor parameters
    header: FileHeader,
    flags: FileFlags,
    pub format: Format,

    // Internals
    index: usize,
    block_size: usize,
}

impl<'a, I: Iterator> Encoder<'a, I> where I: Iterator<Item=PseudoAln> {
    pub fn new_with_format(
        records: &'a mut I,
        query_to_pos: &'a HashMap<String, usize>,
        pos_to_query: &'a HashMap<usize, String>,
        header: FileHeader,
        flags: FileFlags,
        format: Format,
    ) -> Self {
        // Adjust block size to fit within 32-bit address space
        let block_size = ((u32::MAX as u64) / header.n_targets as u64).min(65537_u64) as usize;
        assert!(block_size > 1);
        let block_size = block_size - 1;

        Encoder{
            records, query_to_pos, pos_to_query,
            header, flags, format,
            index: 0, block_size,
        }
    }

    pub fn new(
        records: &'a mut I,
        query_to_pos: &'a HashMap<String, usize>,
        pos_to_query: &'a HashMap<usize, String>,
        header: FileHeader,
        flags: FileFlags,
    ) -> Self {
        Encoder::new_with_format(records, query_to_pos, pos_to_query, header, flags, Format::default())
    }

}

impl<I: Iterator> Encoder<'_, I> where I: Iterator<Item=PseudoAln> {
    pub fn next_block(
        &mut self,
    ) -> Option<Vec<u8>> {
        let mut block_records: Vec<PseudoAln> = Vec::with_capacity(self.block_size);
        for mut record in self.records.by_ref() {
            match &self.format {
                Format::Fulgor => {
                    record.query_id = Some(*self.query_to_pos.get(&record.query_name.clone().unwrap()).unwrap() as u32);
                    record.ones_names = Some(record.ones.as_ref().unwrap().iter().map(|target_idx| {
                        self.flags.target_names[*target_idx as usize].clone()
                    }).collect::<Vec<String>>());
                },
                Format::Themisto => {
                    record.query_name = Some(self.pos_to_query.get(&(record.query_id.unwrap() as usize)).unwrap().clone());
                },
                _ => todo!("Format {:?} is not implemented", self.format),
            };
            block_records.push(record);
            if block_records.len() == self.block_size {
                break;
            }
        }

        if block_records.is_empty() {
            return None
        }

        self.index += block_records.len();
        block_records.sort_by_key(|x| x.query_id.unwrap());

        let mut out: Vec<u8> = Vec::new();
        crate::encode_block(&self.header, &block_records, &mut out).unwrap();

        Some(out)
    }

    pub fn encode_header_and_flags(
        &mut self,
    ) -> Option<Vec<u8>> {
        // TODO Replace unwraps in `encode_header_and_flags`
        let mut flags_bytes = encode_file_flags(&self.flags).unwrap();
        let mut header_bytes = encode_file_header(self.header.n_targets, self.header.n_queries, flags_bytes.len() as u32, 1, 0,0,0).unwrap();

        let mut out: Vec<u8> = Vec::new();
        out.append(&mut flags_bytes);
        out.append(&mut header_bytes);

        Some(out)
    }
}
