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
use crate::headers::file::read_file_header;
use crate::headers::file::read_file_flags;
use crate::headers::block::BlockHeader;
use crate::headers::block::BlockFlags;
use crate::headers::block::read_block_header;
use crate::headers::block::decode_block_flags;
use crate::unpack::inflate_bytes;
use crate::unpack::decode_from_roaring;

use std::io::Read;

pub struct Decoder<'a, R: Read> {
    // Inputs
    conn: &'a mut R,

    header: Option<FileHeader>,
    flags: Option<FileFlags>,

    // Internals
    block: Option<Vec<PseudoAln>>,
    block_index: usize,
    block_header: Option<BlockHeader>,
    block_flags: Option<BlockFlags>,
}

impl<'a, R: Read> Decoder<'a, R> {
    pub fn new(
        conn: &'a mut R,
    ) -> Self {

        let header = read_file_header(conn).unwrap();
        let flags = read_file_flags(&header, conn).unwrap();

        Decoder{
            conn,
            header: Some(header), flags: Some(flags),
            block: None, block_index: 0_usize, block_header: None, block_flags: None,
        }
    }
}

impl<R: Read> Decoder<'_, R> {
    pub fn next_block(
        &mut self,
    ) -> Option<Vec<PseudoAln>> {
        let block_header = read_block_header(self.conn).unwrap();
        let mut bytes: Vec<u8> = vec![0; block_header.deflated_len as usize];
        self.conn.read_exact(&mut bytes).unwrap();

        bytes = inflate_bytes(&bytes).unwrap();
        bytes = inflate_bytes(&bytes).unwrap();

        let block_flags = decode_block_flags(&bytes[(block_header.block_len as usize)..bytes.len()]).unwrap();
        let alns = decode_from_roaring(self.flags.as_ref().unwrap(), &block_header, &block_flags, self.header.as_ref().unwrap().n_targets, &bytes[0..(block_header.block_len as usize)]).unwrap();

        self.block_header = Some(block_header);
        self.block_flags = Some(block_flags);

        Some(alns)

    }
}

impl<R: Read> Iterator for Decoder<'_, R> {
    type Item = PseudoAln;

    fn next(
        &mut self,
    ) -> Option<PseudoAln> {
        if self.block.is_none() {
            self.block = self.next_block();
            self.block_index = 0;
        }

        let ret = self.block.as_ref()?[self.block_index].clone();

        self.block_index += 1;
        if self.block_index == self.block.as_ref().unwrap().len() {
            self.block = None;
        }

        Some(ret)
    }
}
