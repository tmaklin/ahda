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

//! Encoder implementation for an iterator over set bit indexes.

use crate::headers::file::FileHeader;
use crate::headers::file::FileFlags;
use crate::headers::file::build_header_and_flags;
use crate::headers::file::encode_file_header;
use crate::headers::file::encode_file_flags;
use crate::compression::BitmapType;
use crate::compression::roaring32::pack_block_roaring32;
use crate::compression::roaring64::pack_block_roaring64;

use roaring::RoaringBitmap;
use roaring::RoaringTreemap;

pub struct BitmapEncoder<'a, I: Iterator> where I: Iterator<Item=u64> {
    // Input iterator
    set_bits: &'a mut I,
    end: bool,

    // These are given as construtor parameters
    header: FileHeader,
    flags: FileFlags,
    queries: Vec<String>,

    // Internals
    block_size: usize,
    blocks_written: usize,
    bits_buffer: Vec<u64>,
    last_idx: usize,
}

impl<'a, I: Iterator> BitmapEncoder<'a, I> where I: Iterator<Item=u64> {
    pub fn new(
        set_bits: &'a mut I,
        targets: &[String],
        queries: &[String],
        sample_name: &str,
    ) -> Self {
        // TODO `set_bits` must be sorted

        let (header, flags) = build_header_and_flags(targets, queries, sample_name).unwrap();

        // Adjust block size to fit within 32-bit address space
        let block_size = ((u32::MAX as u64) / header.n_targets as u64).min(65537_u64) as usize;
        assert!(block_size > 1);
        let block_size = block_size - 1;

        BitmapEncoder{
            set_bits, end: false,
            header, flags,
            queries: queries.to_vec(),
            block_size, blocks_written: 0_usize,
            bits_buffer: Vec::new(), last_idx: 0_usize,
        }
    }
}

impl<I: Iterator> BitmapEncoder<'_, I> where I: Iterator<Item=u64> {
    pub fn encode_header_and_flags(
        &mut self,
    ) -> Option<Vec<u8>> {
        // TODO Replace unwraps in `encode_header_and_flags`
        let mut flags_bytes = encode_file_flags(&self.flags).unwrap();
        let mut header_bytes = encode_file_header(&self.header).unwrap();

        let mut out: Vec<u8> = Vec::new();
        out.append(&mut header_bytes);
        out.append(&mut flags_bytes);

        Some(out)
    }

    pub fn set_block_size(
        &mut self,
        block_size: usize
    ) {
        let new_block_size = block_size.min(65536_usize);
        assert!(new_block_size > 1);
        self.block_size = new_block_size;
    }

    pub fn build_roaring32(
        &mut self
    ) -> Option<RoaringBitmap> {
        if !self.bits_buffer.is_empty() && self.end {
            let bits = self.bits_buffer.iter().map(|x| *x as u32);
            let bitmap = RoaringBitmap::from_iter(bits);
            self.bits_buffer.clear();
            Some(bitmap)
        } else if !self.bits_buffer.is_empty() {
            let bits = self.bits_buffer.iter().take(self.bits_buffer.len() - 2).map(|x| *x as u32);
            let bitmap = RoaringBitmap::from_iter(bits);
            self.bits_buffer = self.bits_buffer[(self.bits_buffer.len() - 2)..self.bits_buffer.len()].to_vec();
            Some(bitmap)
        } else if self.last_idx < self.header.n_queries as usize && self.end {
            Some(RoaringBitmap::new())
        } else {
            None
        }
    }

    pub fn build_roaring64(
        &mut self
    ) -> Option<RoaringTreemap> {
        if !self.bits_buffer.is_empty() && self.end {
            let bits = self.bits_buffer.iter();
            let bitmap = RoaringTreemap::from_iter(bits);
            self.bits_buffer.clear();
            Some(bitmap)
        } else if !self.bits_buffer.is_empty() {
            let bits = self.bits_buffer.iter().take(self.bits_buffer.len() - 2);
            let bitmap = RoaringTreemap::from_iter(bits);
            self.bits_buffer = self.bits_buffer[(self.bits_buffer.len() - 2)..self.bits_buffer.len()].to_vec();
            Some(bitmap)
        } else if self.last_idx < self.header.n_queries as usize && self.end {
            Some(RoaringTreemap::new())
        } else {
            None
        }
    }
}

impl<I: Iterator> Iterator for BitmapEncoder<'_, I> where I: Iterator<Item=u64> {
    type Item = Vec<u8>;

    fn next(
        &mut self,
    ) -> Option<Vec<u8>> {
        let end_idx = ((self.blocks_written + 1) * self.block_size).min(self.header.n_queries as usize) as u64;
        let n_targets = self.header.n_targets as u64;
        loop {
            if let Some(next_idx) = self.set_bits.next() {
                self.bits_buffer.push(next_idx);
                if next_idx > end_idx * n_targets {
                    break;
                }
            } else {
                self.end = true;
                break;
            }
        }

        let bytes = match BitmapType::from_u16(self.header.bitmap_type).unwrap() {
            BitmapType::Roaring32 => {
                let start_idx = self.blocks_written * self.block_size;
                let block_ids = ((start_idx as u32)..(end_idx as u32)).collect::<Vec<u32>>();
                self.blocks_written += 1;
                self.last_idx = end_idx as usize;
                let bitmap = self.build_roaring32()?;
                pack_block_roaring32(&self.queries[start_idx..(end_idx.try_into().unwrap())], &block_ids, &bitmap).unwrap()
            },
            BitmapType::Roaring64 => {
                let start_idx = self.blocks_written * self.block_size;
                let block_ids = ((start_idx as u32)..(end_idx as u32)).collect::<Vec<u32>>();
                self.blocks_written += 1;
                self.last_idx = end_idx as usize;
                let bitmap = self.build_roaring64()?;
                pack_block_roaring64(&self.queries[start_idx..(end_idx.try_into().unwrap())], &block_ids, &bitmap).unwrap()
            }
        };

        Some(bytes)
    }

}

#[cfg(test)]
mod tests {

    #[test]
    fn encode_header_and_flags() {
        use super::BitmapEncoder;

        let data = vec![0_u64, 2, 4, 5, 7];

        let expected = vec![2, 0, 0, 0, 5, 0, 0, 0, 36, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 10, 69, 82, 82, 52, 48, 51, 53, 49, 50, 54, 2, 9, 99, 104, 114, 46, 102, 97, 115, 116, 97, 13, 112, 108, 97, 115, 109, 105, 100, 46, 102, 97, 115, 116, 97];

        let targets = vec!["chr.fasta".to_string(), "plasmid.fasta".to_string()];
        let queries = vec!["ERR4035126.1".to_string(), "ERR4035126.2".to_string(), "ERR4035126.651903".to_string(), "ERR4035126.7543".to_string(), "ERR4035126.16".to_string()];
        let query_name ="ERR4035126".to_string();

        let mut tmp = data.into_iter();
        let mut encoder = BitmapEncoder::new(&mut tmp, &targets, &queries, &query_name);

        let got = encoder.encode_header_and_flags().unwrap();

        assert_eq!(got, expected);
    }

    #[test]
    fn next() {
        use super::BitmapEncoder;

        let data = vec![0_u64, 2, 4, 5, 7];

        let expected = vec![5, 0, 0, 0, 103, 0, 0, 0, 40, 0, 0, 0, 63, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 99, 229, 113, 13, 10, 50, 49, 48, 54, 53, 52, 50, 211, 51, 68, 230, 24, 9, 34, 113, 204, 76, 13, 45, 13, 140, 249, 145, 68, 204, 77, 77, 140, 121, 145, 245, 154, 177, 50, 48, 50, 49, 179, 0, 0, 164, 198, 115, 218, 81, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 179, 50, 96, 96, 96, 100, 0, 1, 22, 6, 1, 48, 205, 196, 192, 194, 192, 202, 192, 206, 0, 0, 47, 109, 177, 38, 26, 0, 0, 0];

        let targets = vec!["chr.fasta".to_string(), "plasmid.fasta".to_string()];
        let queries = vec!["ERR4035126.1".to_string(), "ERR4035126.2".to_string(), "ERR4035126.651903".to_string(), "ERR4035126.7543".to_string(), "ERR4035126.16".to_string()];
        let query_name ="ERR4035126".to_string();

        let mut tmp = data.into_iter();
        let mut encoder = BitmapEncoder::new(&mut tmp, &targets, &queries, &query_name);
        encoder.set_block_size(1000);

        let got = encoder.next().unwrap();

        assert_eq!(got, expected);
    }

    #[test]
    fn encode_three_blocks_with_next() {
        use super::BitmapEncoder;

        let data = vec![0_u64, 2, 4, 5, 7];

        let expected: Vec<u8> = vec![2, 0, 0, 0, 5, 0, 0, 0, 36, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 10, 69, 82, 82, 52, 48, 51, 53, 49, 50, 54, 2, 9, 99, 104, 114, 46, 102, 97, 115, 116, 97, 13, 112, 108, 97, 115, 109, 105, 100, 46, 102, 97, 115, 116, 97, 2, 0, 0, 0, 74, 0, 0, 0, 34, 0, 0, 0, 40, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 99, 226, 113, 13, 10, 50, 49, 48, 54, 53, 52, 50, 211, 51, 68, 230, 24, 49, 49, 48, 2, 0, 190, 252, 200, 192, 30, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 179, 50, 96, 96, 96, 100, 0, 1, 70, 6, 1, 48, 205, 196, 0, 0, 133, 36, 27, 152, 20, 0, 0, 0, 2, 0, 0, 0, 88, 0, 0, 0, 39, 0, 0, 0, 49, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 99, 18, 116, 13, 10, 50, 49, 48, 54, 53, 52, 50, 211, 51, 51, 53, 180, 52, 48, 230, 71, 18, 49, 55, 53, 49, 102, 98, 98, 6, 0, 10, 60, 125, 12, 38, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 179, 50, 96, 96, 96, 100, 0, 1, 38, 6, 1, 6, 6, 6, 22, 6, 86, 6, 118, 6, 0, 163, 60, 183, 5, 22, 0, 0, 0, 1, 0, 0, 0, 61, 0, 0, 0, 24, 0, 0, 0, 37, 0, 0, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 99, 228, 117, 13, 10, 50, 49, 48, 54, 53, 52, 50, 211, 51, 52, 99, 100, 1, 0, 105, 171, 165, 101, 17, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 179, 50, 96, 0, 3, 0, 142, 53, 76, 217, 8, 0, 0, 0];

        let targets = vec!["chr.fasta".to_string(), "plasmid.fasta".to_string()];
        let queries = vec!["ERR4035126.1".to_string(), "ERR4035126.2".to_string(), "ERR4035126.651903".to_string(), "ERR4035126.7543".to_string(), "ERR4035126.16".to_string()];
        let query_name ="ERR4035126".to_string();

        let mut tmp = data.into_iter();
        let mut encoder = BitmapEncoder::new(&mut tmp, &targets, &queries, &query_name);
        encoder.set_block_size(2);

        let mut got: Vec<u8> = Vec::new();
        got.append(&mut encoder.encode_header_and_flags().unwrap());
        for block in encoder.by_ref() {
            got.append(&mut block.clone());
        }

        assert_eq!(got, expected);
    }
}
