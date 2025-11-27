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
use crate::headers::file::FileHeader;
use crate::headers::file::FileFlags;
use crate::headers::file::encode_file_header;
use crate::headers::file::encode_file_flags;

use std::collections::HashMap;

// TODO records should be anything that implements `next`
pub struct Encoder<'a, I: Iterator> where I: Iterator<Item=PseudoAln> {
    // Inputs
    records: &'a mut I,
    query_to_pos: HashMap<String, usize>,
    pos_to_query: HashMap<usize, String>,

    // These are given as construtor parameters
    header: Option<FileHeader>,
    flags: Option<FileFlags>,

    // Internals
    block: Option<Vec<u8>>,
    block_index: usize,
    block_size: usize,
    blocks_written: usize,
}

impl<'a, I: Iterator> Encoder<'a, I> where I: Iterator<Item=PseudoAln> {
    pub fn new(
        records: &'a mut I,
        targets: &[String],
        queries: &[String],
        sample_name: &str,
    ) -> Self {

        let mut query_to_pos: HashMap<String, usize> = HashMap::new();
        let mut pos_to_query: HashMap<usize, String> = HashMap::new();
        queries.iter().enumerate().for_each(|(idx, query)| {
            query_to_pos.insert(query.clone(), idx);
            pos_to_query.insert(idx, query.clone());
        });

        let flags = FileFlags{ target_names: targets.to_vec(), query_name: sample_name.to_string() };
        let flags_bytes = crate::headers::file::encode_file_flags(&flags).unwrap();
        let header = FileHeader{ n_targets: targets.len() as u32, n_queries: query_to_pos.len() as u32, flags_len: flags_bytes.len() as u32, format: 1_u16, ph2: 0, ph3: 0, ph4: 0 };

        // Adjust block size to fit within 32-bit address space
        let block_size = ((u32::MAX as u64) / header.n_targets as u64).min(65537_u64) as usize;
        assert!(block_size > 1);
        let block_size = block_size - 1;

        Encoder{
            records, query_to_pos, pos_to_query,
            header: Some(header), flags: Some(flags),
            block: Some(Vec::new()), block_index: 0_usize, block_size, blocks_written: 0_usize,
        }
    }
}

impl<I: Iterator> Encoder<'_, I> where I: Iterator<Item=PseudoAln> {
    pub fn next_block(
        &mut self,
    ) -> Option<Vec<u8>> {
        let mut block_records: Vec<PseudoAln> = Vec::with_capacity(self.block_size);
        for mut record in self.records.by_ref() {
            record.query_id = if record.query_id.is_some() { record.query_id } else { Some(*self.query_to_pos.get(&record.query_name.clone().unwrap()).unwrap() as u32) };
            record.query_name = if record.query_name.is_some() { record.query_name } else { Some(self.pos_to_query.get(&(record.query_id.unwrap() as usize)).unwrap().clone()) };

            record.ones_names = if record.ones_names.is_some() { record.ones_names } else {
                Some(record.ones.as_ref().unwrap().iter().map(|target_idx| {
                        self.flags.as_ref().unwrap().target_names[*target_idx as usize].clone()
                }).collect::<Vec<String>>())};

            block_records.push(record);
            if block_records.len() == self.block_size {
                break;
            }
        }

        if block_records.is_empty() {
            return None
        }

        block_records.sort_by_key(|x| x.query_id.unwrap());

        let mut out: Vec<u8> = Vec::new();
        crate::encode_block(self.header.as_ref().unwrap(), &block_records, &mut out).unwrap();

        self.blocks_written += 1;

        Some(out)
    }

    pub fn encode_header_and_flags(
        &mut self,
    ) -> Option<Vec<u8>> {
        // TODO Replace unwraps in `encode_header_and_flags`
        let mut flags_bytes = encode_file_flags(self.flags.as_ref().unwrap()).unwrap();
        let mut header_bytes = encode_file_header(self.header.as_ref().unwrap().n_targets, self.header.as_ref().unwrap().n_queries, flags_bytes.len() as u32, 1, 0,0,0).unwrap();

        let mut out: Vec<u8> = Vec::new();
        out.append(&mut header_bytes);
        out.append(&mut flags_bytes);

        Some(out)
    }
}

impl<I: Iterator> Iterator for Encoder<'_, I> where I: Iterator<Item=PseudoAln> {
    type Item = u8;

    fn next(
        &mut self,
    ) -> Option<u8> {
        if self.blocks_written == 0 {
            self.block = self.encode_header_and_flags();
            self.blocks_written += 1;
        } else if self.block.is_none() {
            self.block = self.next_block();
            self.block_index = 0;
        }

        let ret = self.block.as_ref()?[self.block_index];

        self.block_index += 1;
        if self.block_index == self.block.as_ref().unwrap().len() {
            self.block = None
        }

        Some(ret)
    }

}
