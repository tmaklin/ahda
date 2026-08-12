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

//! File and block headers used in the encoded format.
//!
//! Consists of [FileHeader](file::FileHeader) and
//! [BlockHeader](block::BlockHeader) structs which contain fields that must be
//! filled and must contain the specific records in order to create a valid
//! .ahda record.
//!
//! In addition to the headers, .ahda records contain
//! [FileFlags](file::FileFlags) and [BlockFlags](block::BlockFlags) which are
//! more flexible in their contents. These structs include variable length
//! information about the encoded data. Their encoded length must be recorded in
//! [FileHeader](file::FileHeader) or [BlockHeader](block::BlockHeader).
//!
//! ## File header and flags
//!
//! ### FileHeader
//!
//! A FileHeader must contain this information:
//!
//! - Bytes identifying the data as a .ahda file.
//! - Bytes providing the ahda library version.
//! - The metadata compression method used for FileFlags.
//! - Fields that must be present in every BlockFlags that follows.
//! - Number of target sequences in the alignment.
//! - Number of query sequences in the alignment. This may be 0 if the number was not known in advance.
//! - Type of bitmap stored in the blocks. This may differ for each block if they were not generated with the ahda encode API.
//! - Number of records stored in each block. This may be lower for each individual block.
//! - The number of bytes containing the FileFlags that follow the header.
//!
//! An encoded FileHeader is always 32 bytes long and appears at the start of a
//! valid .ahda record.
//!
//! ### FileFlags
//!
//! FileFlags must contain this information:
//!
//! - A name identifying the query file.
//! - Names of the alignment target sequences.
//!
//! ## Block header and flags
//!
//! ### BlockHeader
//!
//! A BlockHeader must contain the following information:
//!
//! - Number of records stored in this block.
//! - The metadata compression method used for BlockFlags.
//! - Type of bitmap stored in this block.
//! - A 1 byte unused placeholder value.
//! - The number of bytes in the block contents that follow the BlockFlags bytes.
//! - Number of bytes containing the BlockFlags that follow the header.
//! - Fields that are present in the BlockFlags.
//! - A 2 byte unused placeholder value.
//! - A 8 byte unused placeholder value.
//!
//! An encoded BlockHeader is always 32 bytes long and appears at the start of a
//! valid .ahda block.
//!
//! ### BlockFlags
//!
//! BlockFlags may contain this information:
//!
//! - Names of the query sequences, eg. the names identifying the reads in a .fastq file.
//! - Indexes of the same query sequences in the original input.
//!
//! The current implementation assumes that the query indexes are always present
//! if the record was generated using the ahda library.
//!

pub mod block;
pub mod file;
