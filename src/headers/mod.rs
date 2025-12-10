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
//! In addition to the headers, .ahda records may contain
//! [FileFlags](file::FileFlags) and [BlockFlags](block::BlockFlags) which are
//! more flexible in their contents. These structs may include any information
//! but their encoded length must be recorded in [FileHeader](file::FileHeader)
//! or [BlockHeader](block::BlockHeader).
//!
//! ## File header and flags
//!
//! ### FileHeader
//!
//! A FileHeader must contain this information:
//!
//! - Number of alignment targets.
//! - Total number of query sequences.
//! - Length of the FileFlags block (bytes).
//! - Input format.
//!
//! In addition, the header contains three placeholder values (8 + 8 + 2 bytes)
//! that are currently not used.
//!
//! An encoded FileHeader is always 32 bytes long and appears at the start of a
//! valid .ahda record.
//!
//! ### FileFlags
//!
//! A FileFlags should contain this information:
//!
//! - A name identifying the query file.
//! - Names of the alignment target sequences.
//!
//! The flags may also contain other information. In this case, a custom
//! implementation of the flags and the associated encoding/decoding functions
//! should be used.
//!
//! ## Block header and flags
//!
//! ### BlockHeader
//!
//! A BlockHeader must contain the following information:
//!
//! - Number of records in the block.
//! - Length of the rest of the block (bytes). This includes the BlockFlags section.
//! - Length of the BlockFlags section (bytes).
//! - Start index of the block (this is not used).
//! - Two placeholder values, consisting of 8 and 4 bytes.
//!
//! ### BlockFlags
//!
//! A BlockFlags should contain this information:
//!
//! - Names of the query sequences, eg. the names identifying the reads in a .fastq file.
//! - Indexes of the same query sequences in the original input.
//!
//! The flags may also contain other information, that possibly requires a
//! custom implementation to read and/or write.
//!

pub mod block;
pub mod file;
