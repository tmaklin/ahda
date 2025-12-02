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

use roaring::bitmap::RoaringBitmap;

use std::io::Read;

pub mod bitmap;

pub struct Decoder<'a, R: Read> {
    // Inputs
    conn: &'a mut R,

    header: FileHeader,
    flags: FileFlags,

    // Internals
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
            header, flags,
            block_header: None, block_flags: None,
        }
    }
}

impl<R: Read> Decoder<'_, R> {

    pub fn file_header(
        &self,
    ) -> &FileHeader {
        &self.header
    }

    pub fn file_flags(
        &self,
    ) -> &FileFlags {
        &self.flags
    }
}

impl<R: Read> Iterator for Decoder<'_, R> {
    type Item = Vec<PseudoAln>;

    fn next(
        &mut self,
    ) -> Option<Self::Item> {

        match read_block_header(self.conn) {
            Ok(block_header) => {
                let mut bytes: Vec<u8> = vec![0; block_header.deflated_len as usize];
                self.conn.read_exact(&mut bytes).unwrap();

                bytes = inflate_bytes(&bytes).unwrap();
                bytes = inflate_bytes(&bytes).unwrap();

                let block_flags = decode_block_flags(&bytes[(block_header.block_len as usize)..bytes.len()]).unwrap();
                let bitmap = RoaringBitmap::deserialize_from(&bytes[0..(block_header.block_len as usize)]).unwrap();

                let mut tmp = bitmap.iter();
                let bitmap_decoder = bitmap::BitmapDecoder::new(&mut tmp, self.header.clone(), self.flags.clone(), block_header.clone(), block_flags.clone());
                let mut alns: Vec<PseudoAln> = Vec::new();
                for record in bitmap_decoder {
                    alns.push(record);
                }

                self.block_header = Some(block_header);
                self.block_flags = Some(block_flags);

                Some(alns)
            },
            _ => None,
        }
    }
}
