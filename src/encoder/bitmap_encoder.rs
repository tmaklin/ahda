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
//!
//! Implements a struct that can be used to encode data from any iterator over
//! the aligned target indexes.
//!
//! **Note**: The iterator must be sorted.
//!
//! The aligned target indexes are assumed to index a flattened `n_queries x
//! n_targets` matrix in **row-major** order.
//!
//! For example, an iterator over the flattened `3 x 2` matrix containing the
//! indexes `[0, 2, 5]` implies the following alignments:
//!
//! - First query aligned against target with index 0.
//! - Second query aligned against target with index 0.
//! - Third query aligned against target with index 1.
//!
//! To create a valid .ahda record, [BitmapEncoder::encode_file_header_and_flags] should be
//! called first and its output included as the first bytes in the record. This
//! method encodes the [FileHeader] and [FileFlags] corresponding to the data
//! stored in the BitmapEncoder.
//!
//! Block size can be controlled using [BitmapEncoder::set_block_size]. Larger blocks may
//! result in better compression ratios but require more memory to encode and
//! decode.
//!
//! BitmapEncoder will store the pseudoalignment using a [RoaringBitmap] if the
//! number of target sequences times the number of query sequences is less than
//! 2^32. Otherwise, a [RoaringTreemap] will be used.
//!
//! ## Usage
//!
//! ### Encode a vector of alignment target indexes
//!
//! ```rust
//! use ahda::PseudoAln;
//! use ahda::encoder::bitmap_encoder::BitmapEncoder;
//! use std::io::Cursor;
//!
//! let targets = vec!["chr.fasta".as_bytes().to_vec(), "plasmid.fasta".as_bytes().to_vec()];
//! let name = "sample".as_bytes().to_vec();
//!
//! let set_bits_indexes: Vec<u64> = vec![0, 2, 4, 5, 7];
//! let n_records = 5;
//! let mut iter = set_bits_indexes.into_iter();
//!
//! let mut encoder = BitmapEncoder::new(&mut iter, &targets, &name, n_records).expect("Encoder");
//! encoder.set_block_size(3);
//!
//! let mut header_and_flags: Vec<u8> = encoder.encode_file_header_and_flags().expect("Bytes");
//!
//! let mut all_blocks: Vec<u8> = encoder.flat_map(|block| {
//!     assert!(block.is_ok());
//!     block.unwrap()
//! }).collect();
//!
//! let mut bytes: Vec<u8> = header_and_flags;
//! bytes.append(&mut all_blocks);
//!
//! let expected_bytes: Vec<u8> = vec![97, 104, 100, 97, 0, 0, 0, 1, 2, 0, 2, 0, 0, 0, 5, 0, 0, 0, 0, 0, 3, 0, 0, 0, 47, 0, 0, 0, 0, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 99, 43, 78, 204, 45, 200, 73, 101, 226, 76, 206, 40, 210, 75, 75, 44, 46, 73, 228, 45, 200, 73, 44, 206, 205, 76, 129, 240, 0, 98, 108, 248, 160, 32, 0, 0, 0, 3, 0, 0, 0, 1, 0, 0, 0, 38, 0, 0, 0, 26, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 99, 96, 100, 102, 96, 100, 2, 0, 144, 119, 2, 105, 6, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 179, 50, 96, 96, 96, 100, 0, 1, 102, 6, 1, 48, 205, 196, 192, 194, 192, 202, 0, 0, 122, 0, 30, 128, 24, 0, 0, 0, 2, 0, 0, 0, 1, 0, 0, 0, 33, 0, 0, 0, 25, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 99, 96, 100, 98, 102, 1, 0, 204, 211, 90, 81, 5, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 179, 50, 96, 96, 96, 100, 128, 0, 1, 6, 6, 6, 118, 6, 0, 71, 48, 17, 238, 18, 0, 0, 0];
//! assert_eq!(&bytes, &expected_bytes);
//!
//! // The alignments can be decoded from `bytes`
//!
//! let mut input: Cursor<Vec<u8>> = Cursor::new(bytes);
//! let (_file_header, _file_flags, alns) = ahda::decode_from_read(&mut input).unwrap();
//!
//! // Note how the `query_name` field has been filled with names of the format <name>.<query_id> because we did not encode the query names.
//!
//! assert_eq!(alns[0], PseudoAln { ones: Some(vec![0]), ones_names: Some(vec![b"chr.fasta".to_vec()]), query_id: Some(0), query_name: Some("sample.1".as_bytes().to_vec()) });
//! assert_eq!(alns[1], PseudoAln { ones: Some(vec![0]), ones_names: Some(vec![b"chr.fasta".to_vec()]), query_id: Some(1), query_name: Some("sample.2".as_bytes().to_vec()) });
//! assert_eq!(alns[2], PseudoAln { ones: Some(vec![0, 1]), ones_names: Some(vec![b"chr.fasta".to_vec(), b"plasmid.fasta".to_vec()]), query_id: Some(2), query_name: Some("sample.3".as_bytes().to_vec()) });
//! assert_eq!(alns[3], PseudoAln { ones: Some(vec![1]), ones_names: Some(vec![b"plasmid.fasta".to_vec()]), query_id: Some(3), query_name: Some("sample.4".as_bytes().to_vec()) });
//! assert_eq!(alns[4], PseudoAln { ones: Some(vec![]), ones_names: Some(vec![]), query_id: Some(4), query_name: Some("sample.5".as_bytes().to_vec()) });
//! assert_eq!(alns.len(), 5);
//! ```
//!
//! ### Encode and add query names
//!
//! ```rust
//! use ahda::PseudoAln;
//! use ahda::encoder::bitmap_encoder::BitmapEncoder;
//! use std::io::Cursor;
//!
//! let targets = vec!["chr.fasta".as_bytes().to_vec(), "plasmid.fasta".as_bytes().to_vec()];
//! let name = "sample".as_bytes().to_vec();
//!
//! let set_bits_indexes: Vec<u64> = vec![0, 2, 4, 5, 7];
//! let n_records = 5;
//! let mut iter = set_bits_indexes.into_iter();
//!
//! let query_names: Vec<Vec<u8>> = vec![
//!     b"r1".to_vec(),
//!     b"r2".to_vec(),
//!     b"r651903".to_vec(),
//!     b"r7543".to_vec(),
//!     b"r16".to_vec(),
//! ];
//!
//! let mut encoder = BitmapEncoder::new(&mut iter, &targets, &name, n_records).expect("Encoder");
//! encoder.set_block_size(3);
//! encoder.set_query_names(&query_names);
//!
//! let mut header_and_flags: Vec<u8> = encoder.encode_file_header_and_flags().expect("Bytes");
//!
//! let mut all_blocks: Vec<u8> = encoder.flat_map(|block| {
//!     assert!(block.is_ok());
//!     block.unwrap()
//! }).collect();
//!
//! let mut bytes: Vec<u8> = header_and_flags;
//! bytes.append(&mut all_blocks);
//!
//! let expected_bytes: Vec<u8> = vec![97, 104, 100, 97, 0, 0, 0, 1, 3, 0, 2, 0, 0, 0, 5, 0, 0, 0, 0, 0, 3, 0, 0, 0, 47, 0, 0, 0, 0, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 99, 43, 78, 204, 45, 200, 73, 101, 226, 76, 206, 40, 210, 75, 75, 44, 46, 73, 228, 45, 200, 73, 44, 206, 205, 76, 129, 240, 0, 98, 108, 248, 160, 32, 0, 0, 0, 3, 0, 0, 0, 1, 0, 0, 0, 38, 0, 0, 0, 41, 0, 0, 0, 0, 0, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 99, 100, 102, 42, 50, 100, 42, 50, 98, 47, 50, 51, 53, 180, 52, 48, 102, 100, 102, 96, 100, 2, 0, 211, 23, 107, 190, 21, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 179, 50, 96, 96, 96, 100, 0, 1, 102, 6, 1, 48, 205, 196, 192, 194, 192, 202, 0, 0, 122, 0, 30, 128, 24, 0, 0, 0, 2, 0, 0, 0, 1, 0, 0, 0, 33, 0, 0, 0, 36, 0, 0, 0, 0, 0, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 99, 100, 98, 45, 50, 55, 53, 49, 102, 46, 50, 52, 99, 100, 98, 102, 1, 0, 105, 81, 49, 120, 16, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 179, 50, 96, 96, 96, 100, 128, 0, 1, 6, 6, 6, 118, 6, 0, 71, 48, 17, 238, 18, 0, 0, 0];
//! assert_eq!(bytes, expected_bytes);
//!
//! // The alignments can be decoded from `bytes`
//!
//! let mut input: Cursor<Vec<u8>> = Cursor::new(bytes);
//! let (_file_header, _file_flags, alns) = ahda::decode_from_read(&mut input).unwrap();
//!
//! // Note how the `query_name` field now contains the names we supplied via `set_query_names(&query_names)`
//!
//! assert_eq!(alns[0], PseudoAln { ones: Some(vec![0]), ones_names: Some(vec![b"chr.fasta".to_vec()]), query_id: Some(0), query_name: Some("r1".as_bytes().to_vec()) });
//! assert_eq!(alns[1], PseudoAln { ones: Some(vec![0]), ones_names: Some(vec![b"chr.fasta".to_vec()]), query_id: Some(1), query_name: Some("r2".as_bytes().to_vec()) });
//! assert_eq!(alns[2], PseudoAln { ones: Some(vec![0, 1]), ones_names: Some(vec![b"chr.fasta".to_vec(), b"plasmid.fasta".to_vec()]), query_id: Some(2), query_name: Some("r651903".as_bytes().to_vec()) });
//! assert_eq!(alns[3], PseudoAln { ones: Some(vec![1]), ones_names: Some(vec![b"plasmid.fasta".to_vec()]), query_id: Some(3), query_name: Some("r7543".as_bytes().to_vec()) });
//! assert_eq!(alns[4], PseudoAln { ones: Some(vec![]), ones_names: Some(vec![]), query_id: Some(4), query_name: Some("r16".as_bytes().to_vec()) });
//! assert_eq!(alns.len(), 5);
//! ```

use crate::headers::file::FileHeader;
use crate::headers::file::FileFlags;
use crate::headers::file::build_file_header_and_flags;
use crate::headers::file::encode_file_header;
use crate::headers::file::encode_file_flags;
use crate::compression::BitmapType;
use crate::compression::BitmapHolder;
use crate::compression::MetadataCompression;
use crate::compression::roaringwrapper::pack_block_roaring;

use roaring::RoaringBitmap;
use roaring::RoaringTreemap;

type E = Box<dyn std::error::Error>;

use crate::errors::SetBitsIteratorNotSorted;

pub struct BitmapEncoder<'a, I: Iterator> where I: Iterator<Item=u64> {
    // Input iterator
    set_bits: &'a mut I,
    end: bool,

    // These built from constructor parameters
    header: FileHeader,
    flags: FileFlags,

    // Can be optionally given to the encoder
    query_names: Option<Vec<Vec<u8>>>,

    // Internals
    blocks_written: usize,
    bits_buffer: Vec<u64>,
    last_idx: usize,
    prev_idx: u64,
    bitmap_type: BitmapType,
}

impl<'a, I: Iterator> BitmapEncoder<'a, I> where I: Iterator<Item=u64> {
    /// Construct from a [FileHeader] and [FileFlags]
    ///
    /// ## Usage
    ///
    /// ```rust
    /// use ahda::encoder::bitmap_encoder::BitmapEncoder;
    /// use ahda::headers::file::{FileHeader, FileFlags};
    ///
    /// let header = FileHeader::default();
    /// let flags = FileFlags::default();
    /// let set_bit_idxs: Vec<u64> = Vec::new();
    /// let mut iter = set_bit_idxs.into_iter();
    /// let encoder = BitmapEncoder::new_from_header_and_flags(&mut iter, header, flags);
    /// assert!(encoder.is_ok());
    /// ```
    pub fn new_from_header_and_flags(
        set_bits: &'a mut I,
        header: FileHeader,
        flags: FileFlags,
    ) -> Result<Self, E> {
        let mut file_header = header.clone();
        file_header.fields_present = crate::MASK_QUERY_IDS;
        let bitmap_type = BitmapType::from_u16(file_header.bitmap_type)?;
        Ok(BitmapEncoder{
            set_bits, end: false,
            header: file_header, flags,
            blocks_written: 0_usize,
            bits_buffer: Vec::new(), last_idx: 0_usize,
            prev_idx: 0,
            bitmap_type,
            query_names: None,
        })
    }

    /// Construct from info needed to build the [FileHeader] and [FileFlags].
    ///
    /// ## Usage
    ///
    /// ```rust
    /// use ahda::encoder::bitmap_encoder::BitmapEncoder;
    ///
    /// let targets: Vec<Vec<u8>> = Vec::new();
    /// let sample_name: Vec<u8> = Vec::new();
    /// let n_queries: usize = 0;
    /// let set_bit_idxs: Vec<u64> = Vec::new();
    /// let mut iter = set_bit_idxs.into_iter();
    /// let encoder = BitmapEncoder::new(&mut iter, &targets, &sample_name, n_queries);
    /// assert!(encoder.is_ok());
    /// ```
    pub fn new(
        set_bits: &'a mut I,
        targets: &[Vec<u8>],
        sample_name: &[u8],
        n_queries: usize,
    ) -> Result<Self, E> {
        let (header, flags) = build_file_header_and_flags(targets, n_queries, sample_name, &MetadataCompression::default())?;
        Self::new_from_header_and_flags(set_bits, header, flags)
    }

    /// Update `fields_present` in stored FileHeader.
    ///
    /// [MASK_QUERY_IDS](crate::MASK_QUERY_IDS) is always set,
    /// [MASK_QUERIES](crate::MASK_QUERIES) will be set if query names have been
    /// supplied via [BitmapEncoder::set_query_names].
    fn update_fields_present(
        &mut self,
    ) {
        if self.query_names.is_some() {
            self.header.fields_present |= crate::MASK_QUERIES;
        }
        if self.query_names.is_none() {
            self.header.fields_present = crate::MASK_QUERY_IDS;
        }
    }

    /// Make the encoder encode the query names for each block's
    /// [BlockFlags](crate::headers::block::BlockFlags).
    pub fn set_query_names(
        &mut self,
        query_names: &[Vec<u8>],
    ) {
        self.query_names = Some(query_names.to_vec());
        self.update_fields_present();
    }

    /// Change the compression method for [FileFlags] and
    /// [BlockFlags](crate::headers::block::BlockFlags), see
    /// [crate::compression::MetadataCompression] for available options.
    pub fn set_metadata_compression(
        &mut self,
        metadata_compression: &MetadataCompression,
    ) {
        self.header.metadata_compression = MetadataCompression::to_u8(metadata_compression);
    }
}

impl<I: Iterator> BitmapEncoder<'_, I> where I: Iterator<Item=u64> {
    /// Encode the [FileHeader] and [FileFlags] and return the encoded bytes.
    ///
    /// Will update the `fields_present` field of FileHeader if called before
    /// writing any blocks with [BitmapEncoder::next].
    ///
    /// ## Usage
    ///
    /// ```rust
    /// use ahda::encoder::bitmap_encoder::BitmapEncoder;
    ///
    /// let targets: Vec<Vec<u8>> = Vec::new();
    /// let sample_name: Vec<u8> = Vec::new();
    /// let n_queries: usize = 0;
    /// let set_bit_idxs: Vec<u64> = Vec::new();
    /// let mut iter = set_bit_idxs.into_iter();
    /// let mut encoder = BitmapEncoder::new(&mut iter, &targets, &sample_name, n_queries).expect("Encoder");
    ///
    /// let bytes = encoder.encode_file_header_and_flags();
    /// assert!(bytes.is_ok());
    ///
    /// let expected_bytes: Vec<u8> = vec![97, 104, 100, 97, 0, 0, 0, 1, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 22, 0, 0, 0, 0, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 99, 96, 0, 0, 255, 18, 217, 65, 2, 0, 0, 0];
    /// assert_eq!(bytes.unwrap(), expected_bytes);
    /// ```
    pub fn encode_file_header_and_flags(
        &mut self,
    ) -> Result<Vec<u8>, E> {
        if self.blocks_written == 0 {
            self.update_fields_present();
        }
        let mut flags_bytes = encode_file_flags(&self.flags, &MetadataCompression::from_u8(self.header.metadata_compression)?)?;
        self.header.flags_len = flags_bytes.len() as u64;
        let mut header_bytes = encode_file_header(&self.header)?;

        let mut out: Vec<u8> = Vec::new();
        out.append(&mut header_bytes);
        out.append(&mut flags_bytes);

        Ok(out)
    }

    /// Change the number of [PseudoAln](crate::PseudoAln)s stored per block.
    ///
    /// If the underlying bitmap uses a 32-bit address space, the block size is
    /// capped at `u32::MAX / n_targets` regardless of the input argument. For
    /// 64-bit address space, the block size is capped at `u64::MAX /
    /// n_targets`.
    pub fn set_block_size(
        &mut self,
        block_size: usize
    ) -> Result<(), E> {
        let new_block_size: u64 = match BitmapType::from_u16(self.header.bitmap_type)? {
            BitmapType::Roaring32 => {
                let max_block_size = u32::MAX as u64 / self.header.n_targets as u64;
                let block_size_u32: u32 = block_size.try_into().unwrap_or(u32::MAX);
                (block_size_u32 as u64).min(max_block_size)
            },
            BitmapType::Roaring64 => {
                let max_block_size = u64::MAX / self.header.n_targets as u64;
                let block_size_u64 = block_size as u64;
                block_size_u64.min(max_block_size)
            },
        };
        let new_block_size_u32: u32 = new_block_size.try_into().unwrap_or(u32::MAX);
        let min_block_size: u32 = 1;
        self.header.block_size = new_block_size_u32.max(min_block_size);
        Ok(())
    }

    /// Build a RoaringBitmap from the set bit indexes of the current block.
    fn build_roaring32(
        &mut self
    ) -> Option<RoaringBitmap> {
        if !self.bits_buffer.is_empty() && self.end {
            let bits = self.bits_buffer.iter().map(|x| *x as u32);
            let bitmap = RoaringBitmap::from_iter(bits);
            self.bits_buffer.clear();
            Some(bitmap)
        } else if !self.bits_buffer.is_empty() {
            let bits = self.bits_buffer.iter().take(self.bits_buffer.len() - 1).map(|x| *x as u32);
            let bitmap = RoaringBitmap::from_iter(bits);
            self.bits_buffer = self.bits_buffer[(self.bits_buffer.len() - 1)..self.bits_buffer.len()].to_vec();
            Some(bitmap)
        } else if self.last_idx < self.header.n_queries as usize && self.end {
            Some(RoaringBitmap::new())
        } else {
            None
        }
    }

    /// Build a RoaringTreemap from the set bit indexes of the current block.
    fn build_roaring64(
        &mut self
    ) -> Option<RoaringTreemap> {
        if !self.bits_buffer.is_empty() && self.end {
            let bits = self.bits_buffer.iter();
            let bitmap = RoaringTreemap::from_iter(bits);
            self.bits_buffer.clear();
            Some(bitmap)
        } else if !self.bits_buffer.is_empty() {
            let bits = self.bits_buffer.iter().take(self.bits_buffer.len() - 1);
            let bitmap = RoaringTreemap::from_iter(bits);
            self.bits_buffer = self.bits_buffer[(self.bits_buffer.len() - 1)..self.bits_buffer.len()].to_vec();
            Some(bitmap)
        } else if self.last_idx < self.header.n_queries as usize && self.end {
            Some(RoaringTreemap::new())
        } else {
            None
        }
    }
}

impl<I: Iterator> Iterator for BitmapEncoder<'_, I> where I: Iterator<Item=u64> {
    type Item = Result<Vec<u8>, E>;

    /// Encode the next block of records.
    ///
    /// Returns bytes containing the encoded
    /// [BlockHeader](crate::headers::block::BlockHeader),
    /// [BlockFlags](crate::headers::block::BlockFlags), and
    /// [PseudoAln](crate::PseudoAln) vector corresponding to the set bit
    /// indexes for this block.
    ///
    /// ## Errors
    ///
    /// Returns [SetBitsIteratorNotSorted] if the set bit indexes iterator given
    /// to the constructor was not sorted.
    ///
    /// ## Usage
    ///
    /// ```rust
    /// use ahda::encoder::bitmap_encoder::BitmapEncoder;
    ///
    /// let targets = vec!["chr.fasta".as_bytes().to_vec(), "plasmid.fasta".as_bytes().to_vec(), "virus.fasta".as_bytes().to_vec()];
    /// let name = "sample".as_bytes().to_vec();
    ///
    /// let set_bits_indexes: Vec<u64> = vec![0, 2, 4, 5, 7];
    /// let n_records = 5;
    /// let mut iter = set_bits_indexes.into_iter();
    ///
    /// let mut encoder = BitmapEncoder::new(&mut iter, &targets, &name, n_records).expect("Encoder");
    ///
    /// let next_block = encoder.next();
    /// assert!(next_block.is_some());
    /// assert!(next_block.as_ref().unwrap().is_ok());
    ///
    /// let bytes = next_block.unwrap().unwrap();
    /// let expected_bytes: Vec<u8> = vec![5, 0, 0, 0, 1, 0, 0, 0, 40, 0, 0, 0, 28, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 99, 96, 100, 101, 96, 100, 98, 102, 1, 0, 191, 97, 224, 4, 8, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 179, 50, 96, 96, 96, 100, 0, 1, 22, 6, 1, 48, 205, 196, 192, 194, 192, 202, 192, 206, 0, 0, 47, 109, 177, 38, 26, 0, 0, 0];
    ///
    /// assert_eq!(bytes, expected_bytes);
    /// ```
    fn next(
        &mut self,
    ) -> Option<Result<Vec<u8>, E>> {
        assert!(self.header.n_queries > 0);
        let end_idx = ((self.blocks_written + 1) * self.header.block_size as usize).min(self.header.n_queries as usize);
        let n_targets = self.header.n_targets as u64;
        loop {
            if let Some(next_idx) = self.set_bits.next() {
                if next_idx < self.prev_idx {
                    return Some(Err(Box::new(SetBitsIteratorNotSorted{})))
                }
                self.prev_idx = next_idx;

                self.bits_buffer.push(next_idx);
                if next_idx > end_idx as u64 * n_targets {
                    break;
                }
            } else {
                self.end = true;
                break;
            }
        }

        let start_idx = self.blocks_written * self.header.block_size as usize;
        let block_ids = ((start_idx as u32)..(end_idx as u32)).collect::<Vec<u32>>();
        self.blocks_written += 1;
        self.last_idx = end_idx;

        let bitmap = match self.bitmap_type {
            BitmapType::Roaring32 => {
                BitmapHolder::Roaring32(self.build_roaring32()?)
            },
            BitmapType::Roaring64 => {
                BitmapHolder::Roaring64(self.build_roaring64()?)
            }
        };

        let query_names = self.query_names.as_ref().map(|queries| queries[start_idx..end_idx].to_vec());
        let bytes = pack_block_roaring(&self.header, &block_ids, query_names, bitmap);

        match bytes {
            Ok(bytes) => Some(Ok(bytes)),
            Err(e) => Some(Err(e)),
        }
    }

}

#[cfg(test)]
mod tests {

    #[test]
    fn encode_file_header_and_flags() {
        use super::BitmapEncoder;
        use crate::compression::MetadataCompression;

        let data = vec![0_u64, 2, 4, 5, 7];

        let expected = vec![97, 104, 100, 97, 0, 0, 0, 0, 3, 0, 2, 0, 0, 0, 5, 0, 0, 0, 0, 0, 0, 0, 1, 0, 36, 0, 0, 0, 0, 0, 0, 0, 10, 69, 82, 82, 52, 48, 51, 53, 49, 50, 54, 2, 9, 99, 104, 114, 46, 102, 97, 115, 116, 97, 13, 112, 108, 97, 115, 109, 105, 100, 46, 102, 97, 115, 116, 97];

        let targets = vec!["chr.fasta".as_bytes().to_vec(), "plasmid.fasta".as_bytes().to_vec()];
        let queries = vec!["ERR4035126.1".as_bytes().to_vec(), "ERR4035126.2".as_bytes().to_vec(), "ERR4035126.651903".as_bytes().to_vec(), "ERR4035126.7543".as_bytes().to_vec(), "ERR4035126.16".as_bytes().to_vec()];
        let query_name ="ERR4035126".as_bytes().to_vec();
        let n_queries = queries.len();

        let mut tmp = data.into_iter();
        let compression = MetadataCompression::BincodeStandard;
        let mut encoder = BitmapEncoder::new(&mut tmp, &targets, &query_name, n_queries).unwrap();
        encoder.set_metadata_compression(&compression);
        encoder.set_query_names(&queries);

        let got = encoder.encode_file_header_and_flags().unwrap();

        assert_eq!(got, expected);
    }

    #[test]
    fn encode_file_header_and_flags_without_query_names() {
        use super::BitmapEncoder;
        use crate::compression::MetadataCompression;

        let data = vec![0_u64, 2, 4, 5, 7];

        let expected = vec![97, 104, 100, 97, 0, 0, 0, 0, 2, 0, 2, 0, 0, 0, 5, 0, 0, 0, 0, 0, 0, 0, 1, 0, 36, 0, 0, 0, 0, 0, 0, 0, 10, 69, 82, 82, 52, 48, 51, 53, 49, 50, 54, 2, 9, 99, 104, 114, 46, 102, 97, 115, 116, 97, 13, 112, 108, 97, 115, 109, 105, 100, 46, 102, 97, 115, 116, 97];

        let targets = vec!["chr.fasta".as_bytes().to_vec(), "plasmid.fasta".as_bytes().to_vec()];
        let queries = vec!["ERR4035126.1".as_bytes().to_vec(), "ERR4035126.2".as_bytes().to_vec(), "ERR4035126.651903".as_bytes().to_vec(), "ERR4035126.7543".as_bytes().to_vec(), "ERR4035126.16".as_bytes().to_vec()];
        let query_name ="ERR4035126".as_bytes().to_vec();
        let n_queries = queries.len();

        let mut tmp = data.into_iter();
        let mut encoder = BitmapEncoder::new(&mut tmp, &targets, &query_name, n_queries).unwrap();
        let compression = MetadataCompression::BincodeStandard;
        encoder.set_metadata_compression(&compression);

        let got = encoder.encode_file_header_and_flags().unwrap();

        assert_eq!(got, expected);
    }

    #[test]
    fn next() {
        use crate::compression::MetadataCompression;
        use super::BitmapEncoder;

        let data = vec![0_u64, 2, 4, 5, 7];

        let expected = vec![5, 0, 0, 0, 1, 0, 0, 0, 40, 0, 0, 0, 65, 0, 0, 0, 0, 0, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 99, 100, 229, 113, 13, 10, 50, 49, 48, 54, 53, 52, 50, 211, 51, 68, 230, 24, 9, 34, 113, 204, 76, 13, 45, 13, 140, 249, 145, 68, 204, 77, 77, 140, 121, 145, 245, 154, 49, 178, 50, 48, 50, 49, 179, 0, 0, 22, 232, 102, 239, 83, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 179, 50, 96, 96, 96, 100, 0, 1, 22, 6, 1, 48, 205, 196, 192, 194, 192, 202, 192, 206, 0, 0, 47, 109, 177, 38, 26, 0, 0, 0];

        let targets = vec!["chr.fasta".as_bytes().to_vec(), "plasmid.fasta".as_bytes().to_vec()];
        let queries = vec!["ERR4035126.1".as_bytes().to_vec(), "ERR4035126.2".as_bytes().to_vec(), "ERR4035126.651903".as_bytes().to_vec(), "ERR4035126.7543".as_bytes().to_vec(), "ERR4035126.16".as_bytes().to_vec()];
        let query_name ="ERR4035126".as_bytes().to_vec();
        let n_queries = queries.len();

        let mut tmp = data.into_iter();
        let compression = MetadataCompression::Flate2;
        let mut encoder = BitmapEncoder::new(&mut tmp, &targets, &query_name, n_queries).unwrap();
        encoder.set_query_names(&queries);
        encoder.set_metadata_compression(&compression);
        encoder.set_block_size(1000).unwrap();

        let got = encoder.next().unwrap().expect("Ok");

        assert_eq!(got, expected);
    }

    #[test]
    fn next_errors_on_shuffled_bits() {
        use super::BitmapEncoder;

        let data = vec![7_u64, 0, 2, 5, 4];

        let targets = vec!["chr.fasta".as_bytes().to_vec(), "plasmid.fasta".as_bytes().to_vec()];
        let queries = vec!["ERR4035126.1".as_bytes().to_vec(), "ERR4035126.2".as_bytes().to_vec(), "ERR4035126.651903".as_bytes().to_vec(), "ERR4035126.7543".as_bytes().to_vec(), "ERR4035126.16".as_bytes().to_vec()];
        let query_name ="ERR4035126".as_bytes().to_vec();
        let n_queries = queries.len();

        let mut tmp = data.into_iter();
        let mut encoder = BitmapEncoder::new(&mut tmp, &targets, &query_name, n_queries).unwrap();
        encoder.set_query_names(&queries);
        encoder.set_block_size(1000).unwrap();

        let _ = encoder.next().unwrap();
        let got = encoder.next().unwrap();

        assert!(got.is_err());
    }

    #[test]
    fn encode_three_blocks_with_next() {
        use crate::compression::MetadataCompression;
        use super::BitmapEncoder;

        let data = vec![0_u64, 2, 4, 5, 7];

        let expected: Vec<u8> = vec![97, 104, 100, 97, 0, 0, 0, 0, 3, 0, 2, 0, 0, 0, 5, 0, 0, 0, 0, 0, 2, 0, 0, 0, 36, 0, 0, 0, 0, 0, 0, 0, 10, 69, 82, 82, 52, 48, 51, 53, 49, 50, 54, 2, 9, 99, 104, 114, 46, 102, 97, 115, 116, 97, 13, 112, 108, 97, 115, 109, 105, 100, 46, 102, 97, 115, 116, 97, 2, 0, 0, 0, 1, 0, 0, 0, 34, 0, 0, 0, 42, 0, 0, 0, 0, 0, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 99, 100, 226, 113, 13, 10, 50, 49, 48, 54, 53, 52, 50, 211, 51, 68, 230, 24, 49, 50, 49, 48, 2, 0, 26, 63, 239, 0, 32, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 179, 50, 96, 96, 96, 100, 0, 1, 38, 6, 1, 40, 205, 194, 0, 0, 207, 21, 220, 40, 22, 0, 0, 0, 2, 0, 0, 0, 1, 0, 0, 0, 37, 0, 0, 0, 51, 0, 0, 0, 0, 0, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 99, 100, 18, 116, 13, 10, 50, 49, 48, 54, 53, 52, 50, 211, 51, 51, 53, 180, 52, 48, 230, 71, 18, 49, 55, 53, 49, 102, 100, 98, 98, 6, 0, 108, 239, 38, 102, 40, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 179, 50, 96, 96, 96, 100, 0, 1, 70, 6, 1, 6, 6, 6, 86, 6, 118, 6, 0, 242, 32, 178, 210, 20, 0, 0, 0];

        let targets = vec!["chr.fasta".as_bytes().to_vec(), "plasmid.fasta".as_bytes().to_vec()];
        let queries = vec!["ERR4035126.1".as_bytes().to_vec(), "ERR4035126.2".as_bytes().to_vec(), "ERR4035126.651903".as_bytes().to_vec(), "ERR4035126.7543".as_bytes().to_vec(), "ERR4035126.16".as_bytes().to_vec()];
        let query_name ="ERR4035126".as_bytes().to_vec();
        let n_queries = queries.len();

        let mut tmp = data.into_iter();
        let mut encoder = BitmapEncoder::new(&mut tmp, &targets, &query_name, n_queries).unwrap();
        encoder.set_query_names(&queries);
        encoder.set_block_size(2).unwrap();

        let compression = MetadataCompression::BincodeStandard;
        encoder.set_metadata_compression(&compression);
        let mut got: Vec<u8> = Vec::new();
        got.append(&mut encoder.encode_file_header_and_flags().unwrap());
        let compression = MetadataCompression::Flate2;
        encoder.set_metadata_compression(&compression);
        for block in encoder.by_ref() {
            got.append(&mut block.unwrap().clone());
        }

        assert_eq!(got, expected);
    }

    #[test]
    fn encode_three_blocks_with_next_on_shuffled_bits() {
        use super::BitmapEncoder;

        let data = vec![0_u64, 7, 2, 5, 4];

        let targets = vec!["chr.fasta".as_bytes().to_vec(), "plasmid.fasta".as_bytes().to_vec()];
        let queries = vec!["ERR4035126.1".as_bytes().to_vec(), "ERR4035126.2".as_bytes().to_vec(), "ERR4035126.651903".as_bytes().to_vec(), "ERR4035126.7543".as_bytes().to_vec(), "ERR4035126.16".as_bytes().to_vec()];
        let query_name ="ERR4035126".as_bytes().to_vec();
        let n_queries = queries.len();

        let mut tmp = data.into_iter();
        let mut encoder = BitmapEncoder::new(&mut tmp, &targets, &query_name, n_queries).unwrap();
        encoder.set_query_names(&queries);
        encoder.set_block_size(2).unwrap();

        let blocks_iter = encoder.by_ref();
        let _ = blocks_iter.next().unwrap();
        let got = blocks_iter.next().unwrap();
        assert!(got.is_err());
    }

    #[test]
    fn encode_three_blocks_with_next_without_query_names() {
        use crate::compression::MetadataCompression;
        use super::BitmapEncoder;

        let data = vec![0_u64, 2, 4, 5, 7];

        let expected: Vec<u8> = vec![97, 104, 100, 97, 0, 0, 0, 0, 2, 0, 2, 0, 0, 0, 5, 0, 0, 0, 0, 0, 2, 0, 0, 0, 36, 0, 0, 0, 0, 0, 0, 0, 10, 69, 82, 82, 52, 48, 51, 53, 49, 50, 54, 2, 9, 99, 104, 114, 46, 102, 97, 115, 116, 97, 13, 112, 108, 97, 115, 109, 105, 100, 46, 102, 97, 115, 116, 97, 2, 0, 0, 0, 1, 0, 0, 0, 34, 0, 0, 0, 25, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 99, 96, 100, 98, 96, 4, 0, 128, 116, 29, 10, 5, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 179, 50, 96, 96, 96, 100, 0, 1, 38, 6, 1, 40, 205, 194, 0, 0, 207, 21, 220, 40, 22, 0, 0, 0, 2, 0, 0, 0, 1, 0, 0, 0, 37, 0, 0, 0, 25, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 99, 96, 100, 98, 98, 6, 0, 46, 119, 37, 214, 5, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 179, 50, 96, 96, 96, 100, 0, 1, 70, 6, 1, 6, 6, 6, 86, 6, 118, 6, 0, 242, 32, 178, 210, 20, 0, 0, 0];

        let targets = vec!["chr.fasta".as_bytes().to_vec(), "plasmid.fasta".as_bytes().to_vec()];
        let query_name ="ERR4035126".as_bytes().to_vec();
        let n_queries = 5_usize;

        let mut tmp = data.into_iter();
        let mut encoder = BitmapEncoder::new(&mut tmp, &targets, &query_name, n_queries).unwrap();
        encoder.set_block_size(2).unwrap();

        let compression = MetadataCompression::BincodeStandard;
        encoder.set_metadata_compression(&compression);
        let mut got = encoder.encode_file_header_and_flags().unwrap();
        let compression = MetadataCompression::Flate2;
        encoder.set_metadata_compression(&compression);
        for block in encoder {
            got.append(&mut block.unwrap());
        }

        assert_eq!(got, expected);
    }

}
