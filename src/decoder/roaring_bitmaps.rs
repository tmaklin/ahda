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
use crate::headers::block::BlockHeader;
use crate::headers::block::BlockFlags;

pub struct BitmapDecoder<'a, I: Iterator> where I: Iterator<Item=u32> {
    // Inputs
    bits_iter: &'a mut I,
    index: Option<u32>,
    prev_index: Option<u32>,

    file_header: FileHeader,
    file_flags: FileFlags,

    _block_header: BlockHeader,
    block_flags: BlockFlags,
}

impl<'a, I: Iterator> BitmapDecoder<'a, I> where I: Iterator<Item=u32> {
    pub fn new(
        bits_iter: &'a mut I,
        file_header: FileHeader,
        file_flags: FileFlags,
        block_header: BlockHeader,
        block_flags: BlockFlags,
    ) -> Self {

        BitmapDecoder {
            bits_iter, file_header, file_flags, _block_header: block_header, block_flags,
            index: Some(0), prev_index: None,
        }
    }
}

impl<I: Iterator> Iterator for BitmapDecoder<'_, I> where I: Iterator<Item=u32>{
    type Item = PseudoAln;

    fn next(
        &mut self,
    ) -> Option<Self::Item> {
        let mut ones: Vec<u32> = Vec::with_capacity(self.file_header.n_targets as usize);
        let mut names: Vec<String> = Vec::with_capacity(self.file_header.n_targets as usize);
        let mut query_idx = None;

        for idx in self.bits_iter.by_ref() {
            if self.prev_index.is_some() {
                let target_idx = self.prev_index.unwrap() % self.file_header.n_targets;
                ones.push(target_idx);
                names.push(self.file_flags.target_names[target_idx as usize].clone());
                self.prev_index = None;
            }
            query_idx = Some(idx / self.file_header.n_targets);
            if query_idx.unwrap() == *self.index.as_ref().unwrap() / self.file_header.n_targets {
                let target_idx = idx % self.file_header.n_targets;
                ones.push(target_idx);
                names.push(self.file_flags.target_names[target_idx as usize].clone());
                self.index = Some(idx);
            } else {
                query_idx = Some(self.index.unwrap() / self.file_header.n_targets);
                self.index = Some(idx);
                self.prev_index = Some(idx);
                break;
            }
        }

        if let Some(query_idx) = query_idx {
            Some(PseudoAln {
                ones: Some(ones),
                ones_names: Some(names),
                query_id: Some(query_idx),
                query_name: Some(self.block_flags.queries[query_idx as usize].clone()),
            })
        } else {
            None
        }
    }
}
