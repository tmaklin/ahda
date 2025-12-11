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
use crate::headers::file::encode_file_header;
use crate::headers::file::encode_file_flags;
use crate::encoder::pack_roaring::pack_block;

use roaring::RoaringBitmap;

pub struct BitmapEncoder<'a, I: Iterator> where I: Iterator<Item=u32> {
    // Inputs
    set_bits: &'a mut I,

    // These are given as construtor parameters
    header: FileHeader,
    flags: FileFlags,
    queries: Vec<String>,

    // Internals
    block_size: usize,
    blocks_written: usize,
    bits_buffer: Vec<u32>,
    last_idx: usize,
}

impl<'a, I: Iterator> BitmapEncoder<'a, I> where I: Iterator<Item=u32> {
    pub fn new(
        set_bits: &'a mut I,
        targets: &[String],
        queries: &[String],
        sample_name: &str,
    ) -> Self {
        // TODO `set_bits` must be sorted

        let flags = FileFlags{ target_names: targets.to_vec(), query_name: sample_name.to_string() };
        let flags_bytes = crate::headers::file::encode_file_flags(&flags).unwrap();
        let header = FileHeader{ n_targets: targets.len() as u32, n_queries: queries.len() as u32, flags_len: flags_bytes.len() as u32, format: 1_u16, ph2: 0, ph3: 0, ph4: 0 };

        // Adjust block size to fit within 32-bit address space
        let block_size = ((u32::MAX as u64) / header.n_targets as u64).min(65537_u64) as usize;
        assert!(block_size > 1);
        let block_size = block_size - 1;

        BitmapEncoder{
            set_bits,
            header, flags,
            queries: queries.to_vec(),
            block_size, blocks_written: 0_usize,
            bits_buffer: Vec::new(), last_idx: 0_usize,
        }
    }
}

impl<I: Iterator> BitmapEncoder<'_, I> where I: Iterator<Item=u32> {
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

}

impl<I: Iterator> Iterator for BitmapEncoder<'_, I> where I: Iterator<Item=u32> {
    type Item = Vec<u8>;

    fn next(
        &mut self,
    ) -> Option<Vec<u8>> {
        let mut end = false;
        let end_idx = ((self.blocks_written + 1) * self.block_size).min(self.header.n_queries as usize);
        loop {
            if let Some(next_idx) = self.set_bits.next() {
                self.bits_buffer.push(next_idx);
                if next_idx > end_idx as u32 * self.header.n_targets {
                    break;
                }
            } else {
                end = true;
                break;
            }
        }

        if !self.bits_buffer.is_empty() && end {
            let bits = self.bits_buffer.iter();
            let bitmap = RoaringBitmap::from_iter(bits);
            self.bits_buffer.clear();
            let start_idx = self.blocks_written * self.block_size;
            let block_queries = &self.queries[start_idx..end_idx];
            let block_ids = ((start_idx as u32)..(end_idx as u32)).collect::<Vec<u32>>();
            self.blocks_written += 1;
            self.last_idx = end_idx;
            Some(pack_block(block_queries, &block_ids, &bitmap).unwrap())
        } else if !self.bits_buffer.is_empty() {
            let bits = self.bits_buffer.iter().take(self.bits_buffer.len() - 2);
            let bitmap = RoaringBitmap::from_iter(bits);
            self.bits_buffer = self.bits_buffer[(self.bits_buffer.len() - 2)..self.bits_buffer.len()].to_vec();
            let start_idx = self.blocks_written * self.block_size;
            let block_queries = &self.queries[start_idx..end_idx];
            let block_ids = ((start_idx as u32)..(end_idx as u32)).collect::<Vec<u32>>();
            self.blocks_written += 1;
            self.last_idx = end_idx;
            Some(pack_block(block_queries, &block_ids, &bitmap).unwrap())
        } else if self.last_idx < self.header.n_queries as usize && end {
            let bitmap = RoaringBitmap::new();
            let start_idx = self.blocks_written * self.block_size;
            let block_queries = &self.queries[start_idx..end_idx];
            let block_ids = ((start_idx as u32)..(end_idx as u32)).collect::<Vec<u32>>();
            self.blocks_written += 1;
            self.last_idx = end_idx;
            Some(pack_block(block_queries, &block_ids, &bitmap).unwrap())

        } else {
            None
        }
    }

}

#[cfg(test)]
mod tests {

    #[test]
    fn encode_header_and_flags() {
        use crate::PseudoAln;
        use super::BitmapEncoder;

        let data = vec![0_u32, 2, 4, 5, 7];

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
        use crate::PseudoAln;
        use super::BitmapEncoder;

        let data = vec![0_u32, 2, 4, 5, 7];

        let expected = vec![5, 0, 0, 0, 103, 0, 0, 0, 26, 0, 0, 0, 81, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 147, 239, 230, 96, 0, 131, 255, 155, 141, 18, 18, 18, 82, 24, 24, 197, 216, 24, 13, 206, 30, 57, 112, 232, 192, 169, 3, 231, 14, 156, 122, 44, 37, 146, 146, 148, 144, 147, 149, 145, 178, 44, 189, 227, 140, 161, 144, 203, 163, 25, 51, 165, 162, 164, 36, 62, 43, 119, 206, 152, 61, 75, 226, 179, 210, 107, 211, 228, 212, 132, 148, 164, 52, 70, 134, 146, 247, 91, 214, 102, 51, 48, 48, 0, 0, 206, 10, 209, 169, 83, 0, 0, 0];

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
        use crate::PseudoAln;
        use super::BitmapEncoder;

        use crate::decode_from_read;

        let data = vec![0_u32, 2, 4, 5, 7];

        let expected = vec![2, 0, 0, 0, 5, 0, 0, 0, 36, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 10, 69, 82, 82, 52, 48, 51, 53, 49, 50, 54, 2, 9, 99, 104, 114, 46, 102, 97, 115, 116, 97, 13, 112, 108, 97, 115, 109, 105, 100, 46, 102, 97, 115, 116, 97, 2, 0, 0, 0, 72, 0, 0, 0, 20, 0, 0, 0, 30, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 147, 239, 230, 96, 0, 131, 255, 155, 141, 18, 18, 18, 82, 24, 24, 221, 216, 24, 13, 206, 30, 57, 112, 228, 177, 148, 72, 74, 82, 66, 78, 86, 70, 202, 178, 244, 142, 51, 134, 73, 73, 9, 44, 12, 166, 66, 39, 86, 27, 49, 48, 48, 0, 0, 86, 244, 9, 212, 54, 0, 0, 0, 2, 0, 0, 0, 85, 0, 0, 0, 22, 0, 0, 0, 38, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 147, 239, 230, 96, 0, 131, 255, 155, 141, 18, 18, 18, 82, 24, 24, 213, 216, 24, 217, 216, 216, 196, 216, 194, 216, 202, 216, 212, 28, 175, 47, 80, 16, 102, 78, 14, 118, 86, 54, 182, 53, 14, 118, 246, 102, 78, 174, 83, 17, 44, 14, 22, 78, 86, 83, 75, 99, 144, 109, 179, 60, 99, 195, 192, 192, 0, 0, 99, 234, 9, 73, 68, 0, 0, 0, 1, 0, 0, 0, 58, 0, 0, 0, 8, 0, 0, 0, 17, 0, 0, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 147, 239, 230, 96, 0, 131, 255, 155, 141, 18, 24, 152, 221, 226, 174, 47, 80, 16, 102, 78, 14, 118, 86, 54, 182, 117, 54, 118, 19, 99, 112, 225, 204, 153, 32, 201, 192, 192, 0, 0, 241, 222, 62, 125, 41, 0, 0, 0];

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
