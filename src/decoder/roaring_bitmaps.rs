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
use crate::unpack::decode_from_roaring;

use roaring::RoaringBitmap;

pub struct RoaringDecoder<'a> {
    // Inputs
    bitmap: &'a RoaringBitmap,

    file_header: FileHeader,
    file_flags: FileFlags,

    block_header: BlockHeader,
    block_flags: BlockFlags,
}

impl<'a> RoaringDecoder<'a> {
    pub fn new(
        bitmap: &'a RoaringBitmap,
        file_header: FileHeader,
        file_flags: FileFlags,
        block_header: BlockHeader,
        block_flags: BlockFlags,
    ) -> Self {

        RoaringDecoder {
            bitmap, file_header, file_flags, block_header, block_flags,
        }
    }
}

impl Iterator for RoaringDecoder<'_> {
    type Item = Vec<PseudoAln>;

    fn next(
        &mut self,
    ) -> Option<Self::Item> {

        let alns = decode_from_roaring(self.bitmap, &self.file_flags, &self.block_header, &self.block_flags, self.file_header.n_targets).unwrap();
        Some(alns)
    }
}
