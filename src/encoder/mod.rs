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

pub struct Encoder<'a, I: Iterator> where I: Iterator<Item=PseudoAln> {
    // Inputs
    records: &'a mut I,

    // These are given as construtor parameters
    header: FileHeader,
    flags: FileFlags,

    // Internals
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
        let flags = FileFlags{ target_names: targets.to_vec(), query_name: sample_name.to_string() };
        let flags_bytes = crate::headers::file::encode_file_flags(&flags).unwrap();
        let header = FileHeader{ n_targets: targets.len() as u32, n_queries: queries.len() as u32, flags_len: flags_bytes.len() as u32, format: 1_u16, ph2: 0, ph3: 0, ph4: 0 };

        // Adjust block size to fit within 32-bit address space
        let block_size = ((u32::MAX as u64) / header.n_targets as u64).min(65537_u64) as usize;
        assert!(block_size > 1);
        let block_size = block_size - 1;

        Encoder{
            records,
            header, flags,
            block_size, blocks_written: 0_usize,
        }
    }
}

impl<I: Iterator> Encoder<'_, I> where I: Iterator<Item=PseudoAln> {
    pub fn encode_header_and_flags(
        &mut self,
    ) -> Option<Vec<u8>> {
        // TODO Replace unwraps in `encode_header_and_flags`
        let mut flags_bytes = encode_file_flags(&self.flags).unwrap();
        let mut header_bytes = encode_file_header(self.header.n_targets, self.header.n_queries, flags_bytes.len() as u32, 1, 0,0,0).unwrap();

        let mut out: Vec<u8> = Vec::new();
        out.append(&mut header_bytes);
        out.append(&mut flags_bytes);

        Some(out)
    }
}

impl<I: Iterator> Iterator for Encoder<'_, I> where I: Iterator<Item=PseudoAln> {
    type Item = Vec<u8>;

    fn next(
        &mut self,
    ) -> Option<Vec<u8>> {
        let mut block_records: Vec<PseudoAln> = Vec::with_capacity(self.block_size);
        for record in self.records.by_ref() {
            // TODO Check that all fields are set?
            block_records.push(record);
            if block_records.len() == self.block_size {
                break;
            }
        }

        if block_records.is_empty() {
            return None
        }

        block_records.sort_by_key(|x| x.query_id.unwrap());

        let out = crate::pack::pack(&self.header, &block_records).unwrap();

        self.blocks_written += 1;

        Some(out)
    }

}
