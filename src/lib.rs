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

//! ahda is a library and a command-line client for:
//!
//!   - Converting between plain text pseudoalignment formats output by different tools.
//!   - Compressing and decompressing plain text pseudoalignment data.
//!   - Performing set operations on compressed pseudoalignment data.
//!   - Concatenating compressed pseudoalignment data.
//!
//! The following plain text formats are supported:
//!   - [Bifrost](https://github.com/pmelsted/bifrost)
//!   - [Fulgor](https://github.com/jermp/fulgor)
//!   - [Metagraph](https://github.com/ratschlab/metagraph)
//!   - [SAM](https://samtools.github.io/hts-specs/SAMv1.pdf) (input only)
//!   - [Themisto](https://github.com/algbio/themisto)
//!
//! Internally, ahda uses [roaring bitmaps](https://roaringbitmap.org/) to store
//! the pseudoalignments.
//!
//! ## Usage
//!
//! ### Command line
//!
//! The ahda CLI supports the following subcommands:
//!   - `ahda cat` concatenate compressed pseudoalignment data.
//!   - `ahda convert` convert between supported plain text formats.
//!   - `ahda decode` decompress pseudoalignment data to a supported format.
//!   - `ahda encode` compress pseudoalignment data from a supported format.
//!   - `ahda set` perform set operations on compressed pseudoalignment data.
//!
//! Note that `encode` needs access to the .fastq input file and the names of
//! the pseudoalignment targets. These are required to create an encoded record
//! that can be converted to any of the supported plain text formats, because
//! the plain text formats contain varying levels of information about the input
//! data.
//!
//! ### Rust API
//!
//! The API provides several functions for operating on structs that implement
//! [Read] and/or [Write]. These are meant for use cases where an entire stream
//! should be processed.
//!
//! For use cases requiring access to a single record at a time, the following
//! structs are provided:
//!
//!   - [Decoder](decoder::Decoder): takes a [Read] containing the encoded bytes and decodes them into [PseudoAln].
//!   - [BitmapDecoder](decoder::bitmap::BitmapDecoder): takes an iterator over the indexes of set bits and decodes them into [PseudoAln].
//!   - [Encoder](encoder::Encoder): takes an iterator over [PseudoAln] records and encodes them into a Vec<u8>.
//!   - [Parser](parser::Parser): takes a [Read] containing plain text pseudoalignment bytes and converts them into [PseudoAln].
//!   - [Printer](printer::Printer): takes an iterator over [PseudoAln] records and formats them into plain text data.
//!
//! These structs can additionally be chained together to eg. read encoded data
//! and print it in a plain text format, or to parse plain text data and encode
//! it.
//!
//! See documentation for the appropriate functions or structs for usage examples.
//!
//! ### C++ API
//!
//! ahda provides a C++ API for encoding and decoding pseudoalignment data into
//! memory. The API is available in [cxx_api].
//!
//! Encoding requires converting the pseudoalignment to a flattened form
//! and obtaining the indexes of the set bits (positive alignments) that should
//! be included in a bitmap representation.
//!
//! The encoding API supports writing either a complete .ahda record or one
//! block at a time.
//!
//! Decoding can be performed by reading the bytes of a full .ahda record into
//! memory, or by reading in one block at a time. The output from the decoding
//! API are the indexes of the set bits in the (flattened) bitmap
//! representation.
//!
//! The decoding API also supports reading in the target sequence names, the
//! query sequence names, and the positions of the query sequences in the
//! original query input.
//!
//! ## File format specification
//!
//! The .ahda file format has the following structure:
//!
//! **TODO** Write file format specification
//!

use headers::file::FileHeader;
use headers::file::FileFlags;
use headers::block::BlockFlags;
use headers::block::read_block_header;
use headers::file::read_file_header;
use headers::file::read_file_flags;
use headers::file::encode_file_header;
use headers::file::encode_file_flags;
use compression::roaring32::unpack_block_roaring32;

use std::io::Read;
use std::io::Write;

use roaring::bitmap::RoaringBitmap;

pub mod cxx_api;

pub mod compression;
pub mod headers;
pub mod decoder;
pub mod encoder;
pub mod parser;
pub mod printer;

type E = Box<dyn std::error::Error>;

/// Supported plain text formats.
#[non_exhaustive]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Format {
    #[default] // TODO more sensible default
    Bifrost,
    Fulgor,
    Metagraph,
    SAM,
    Themisto,
}

impl std::str::FromStr for Format {
    type Err = String; // Define an error type for parsing failures

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "bifrost" => Ok(Format::Bifrost),
            "fulgor" => Ok(Format::Fulgor),
            "metagraph" => Ok(Format::Metagraph),
            "sam" => Ok(Format::SAM),
            "themisto" => Ok(Format::Themisto),
            _ => Err(format!("'{}' is not a valid Format", s)),
        }
    }
}

/// Supported set operations for [decode_from_read_into_roaring].
#[non_exhaustive]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum MergeOp {
    #[default]
    Union,
    Intersection,
    Xor,
    Diff,
}

impl std::str::FromStr for MergeOp {
    type Err = String; // Define an error type for parsing failures

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "union" => Ok(MergeOp::Union),
            "intersection" => Ok(MergeOp::Intersection),
            "xor" => Ok(MergeOp::Xor),
            "diff" => Ok(MergeOp::Diff),
            _ => Err(format!("'{}' is not a valid MergeOp", s)),
        }
    }
}

/// A decompressed pseudoalignment record.
///
/// The fields are stored as Option to enable parsing them from incomplete
/// plaintext formats. If an incomplete alignment is parsed without using the
/// ahda API, this data must be filled in to create a valid .ahda record from
/// the encode API calls or with the [Encoder] class.
///
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PseudoAln{
    /// Indexes of positive alignment targets.
    pub ones: Option<Vec<u32>>,
    /// Names of positive alignment targets.
    pub ones_names: Option<Vec<String>>,
    /// Index of the query sequence in the query file.
    pub query_id: Option<u32>,
    /// Name of the query sequence in the query file.
    pub query_name: Option<String>,
}

/// Merge compressed data by concatenating all blocks.
///
/// This simply appends the blocks in input order using [std::io::copy], it does
/// not check or modify the contents of the block header, block flags, or the
/// block data.
///
/// Updates the `n_queries` and `flags_len` fields in [FileHeader] to match the
/// new data.
///
/// Retains the `query_name` and `target_names` fields for [FileFlags] from the
/// first input.
///
/// ## Errors and panics
///
/// Panics if the [file headers](FileHeader) have different
/// number of targets or different target sequence names.
///
/// ## Usage
///
/// ```rust
/// use ahda::{concatenate_from_read_to_write, decode_from_read, encode_to_write};
/// use ahda::PseudoAln;
/// use std::io::{Cursor, Seek};
///
/// // Set up mock inputs
/// let targets = vec!["chr.fasta".to_string(), "plasmid.fasta".to_string()];
/// let queries = vec!["ERR4035126.1".to_string(), "ERR4035126.2".to_string(), "ERR4035126.651903".to_string(), "ERR4035126.7543".to_string(), "ERR4035126.16".to_string()];
/// let name = "ERR4035126".to_string();
///
/// let data_1 = vec![
///     PseudoAln{ones_names: Some(vec!["chr.fasta".to_string()]),  query_id: Some(0), ones: Some(vec![0]), query_name: Some("ERR4035126.1".to_string()) },
///     PseudoAln{ones_names: Some(vec!["chr.fasta".to_string()]),  query_id: Some(1), ones: Some(vec![0]), query_name: Some("ERR4035126.2".to_string()) },
/// ];
/// let data_2 = vec![
///     PseudoAln{ones_names: Some(vec!["plasmid.fasta".to_string()]),  query_id: Some(3), ones: Some(vec![1]), query_name: Some("ERR4035126.7543".to_string()) },
///     PseudoAln{ones_names: Some(vec![]),  query_id: Some(4), ones: Some(vec![]), query_name: Some("ERR4035126.16".to_string()) },
/// ];
///
/// let mut data_bytes_1: Cursor<Vec<u8>> = Cursor::new(Vec::new());
/// let mut data_bytes_2: Cursor<Vec<u8>> = Cursor::new(Vec::new());
///
/// // Encode the mock inputs to data_bytes_1 and data_bytes_2.
/// encode_to_write(&targets, &queries, &name, &data_1, &mut data_bytes_1).unwrap();
/// encode_to_write(&targets, &queries, &name, &data_2, &mut data_bytes_2).unwrap();
///
/// data_bytes_1.rewind();
/// data_bytes_2.rewind();
///
/// // Concatenate the data
/// let mut inputs = vec![data_bytes_1, data_bytes_2];
/// let mut output: Cursor<Vec<u8>> = Cursor::new(Vec::new());
///
/// concatenate_from_read_to_write(&mut inputs, &mut output).unwrap();
/// output.rewind();
///
/// // output contains the alignments from data_1 and data_2
/// let (file_header, _file_flags, data_both) = decode_from_read(&mut output).unwrap();
///
/// assert_eq!(data_both[0..2], data_1);
/// assert_eq!(data_both[2..4], data_2);
/// ```
pub fn concatenate_from_read_to_write<R: Read, W: Write>(
    conns: &mut [R],
    conn_out: &mut W,
) -> Result<(), E> {
    let headers_flags = conns.iter_mut().map(|conn_in| {
        let header = read_file_header(conn_in).unwrap();
        let flags = read_file_flags(&header, conn_in).unwrap();
        (header, flags)
    }).collect::<Vec<(FileHeader, FileFlags)>>();

    let mut n_queries = 0_u32;
    let n_targets = headers_flags[0].0.n_targets;
    let target_names = headers_flags[0].1.target_names.clone();
    let query_name = headers_flags[0].1.query_name.clone();

    headers_flags.iter().for_each(|(header, flags)| {
        n_queries += header.n_queries;
        assert_eq!(n_targets, header.n_targets);
        assert_eq!(target_names, flags.target_names);
    });

    let new_flags = FileFlags { query_name, target_names };
    let new_flags_bytes = encode_file_flags(&new_flags)?;
    let new_header = FileHeader { n_targets, n_queries, flags_len: new_flags_bytes.len() as u32, format: 0, bitmap_type: 0, ph3: 0, ph4: 0 };
    let new_header_bytes = encode_file_header(&new_header)?;
    conn_out.write_all(&new_header_bytes)?;
    conn_out.write_all(&new_flags_bytes)?;

    conns.iter_mut().for_each(|conn_in| {
        std::io::copy(conn_in, conn_out).unwrap();
    });

    conn_out.flush()?;

    Ok(())
}

/// Convert plain text data from [Read] to plain text data to [Write].
///
/// Can read and write to any format supported by [Format].
///
/// ## Usage
///
/// ```rust
/// use ahda::convert_from_read_to_write;
/// use ahda::Format;
/// use std::io::Cursor;
///
/// // Mock themisto formatted data
///
/// // Have this input
/// //   3 0 2
/// //   0 2
/// //   4 0 1 2
/// //   2
///
/// let mut input_bytes: Vec<u8> = Vec::new();
/// input_bytes.append(&mut b"3 0 2\n".to_vec());
/// input_bytes.append(&mut b"0 2\n".to_vec());
/// input_bytes.append(&mut b"4 0 1 2\n".to_vec());
/// input_bytes.append(&mut b"2\n".to_vec());
/// let mut input: Cursor<Vec<u8>> = Cursor::new(input_bytes);
///
/// // Mock inputs
/// let targets = vec!["chr.fasta".to_string(), "plasmid.fasta".to_string(), "virus.fasta".to_string()];
/// let queries = vec!["1".to_string(), "2".to_string(), "651903".to_string(), "7543".to_string(), "16".to_string()];
/// let name = "sample".to_string();
///
/// // Convert to metagraph format
/// let out_format = Format::Metagraph;
/// let mut output: Vec<u8> = Vec::new();
/// convert_from_read_to_write(&targets, &queries, &name, out_format, &mut input, &mut output).unwrap();
///
/// // Expect to get this output:
/// //   3    7543    chr.fasta:virus.fasta
/// //   0    1       virus.fasta
/// //   4    16      chr.fasta:plasmid.fasta:virus.fasta
/// //   2    651903
/// //
/// let mut expected: Vec<u8> = Vec::new();
/// expected.append(&mut b"3\t7543\tchr.fasta:virus.fasta\n".to_vec());
/// expected.append(&mut b"0\t1\tvirus.fasta\n".to_vec());
/// expected.append(&mut b"4\t16\tchr.fasta:plasmid.fasta:virus.fasta\n".to_vec());
/// expected.append(&mut b"2\t651903\t\n".to_vec());
///
/// assert_eq!(output, expected);
/// ```
///
pub fn convert_from_read_to_write<R: Read, W: Write>(
    targets: &[String],
    queries: &[String],
    sample_name: &str,
    format: Format,
    conn_in: &mut R,
    conn_out: &mut W,
) -> Result<(), E> {
    let mut reader = crate::parser::Parser::new(conn_in, targets, queries, sample_name)?;
    let header = reader.file_header().clone();
    let flags = reader.file_flags().clone();
    let mut writer = crate::printer::Printer::new_from_header_and_flags(&mut reader, header, flags, format);
    for record in writer.by_ref() {
        conn_out.write_all(&record)?;
    }
    Ok(())
}

/// Encode from memory to something that implements [Write](std::io::Write).
///
/// ## Errors and panics
///
/// Panics if the input `records` is empty
///
/// ## Usage
/// ```rust
/// use ahda::{encode_to_write, decode_from_read};
/// use ahda::PseudoAln;
/// use std::io::{Cursor, Seek};
///
/// // Mock data
/// let data = vec![
///     PseudoAln{ones_names: Some(vec!["chr.fasta".to_string()]),  query_id: Some(0), ones: Some(vec![0]), query_name: Some("ERR4035126.1".to_string()) },
///     PseudoAln{ones_names: Some(vec!["chr.fasta".to_string()]),  query_id: Some(1), ones: Some(vec![0]), query_name: Some("ERR4035126.2".to_string()) },
/// ];
///
/// let targets = vec!["chr.fasta".to_string(), "plasmid.fasta".to_string()];
/// let queries = vec!["ERR4035126.1".to_string(), "ERR4035126.2".to_string(), "ERR4035126.651903".to_string(), "ERR4035126.7543".to_string(), "ERR4035126.16".to_string()];
/// let name = "ERR4035126".to_string();
///
/// // Encode to `output`
/// let mut output: Cursor<Vec<u8>> = Cursor::new(Vec::new());
/// encode_to_write(&targets, &queries, &name, &data, &mut output).unwrap();
///
/// // `output` can be decoded to get the original data back
/// output.rewind();
/// let (header, flags, decoded_data) = decode_from_read(&mut output).unwrap();
///
/// assert_eq!(decoded_data, data);
/// ```
///
pub fn encode_to_write<W: Write>(
    targets: &[String],
    queries: &[String],
    sample_name: &str,
    records: &[PseudoAln],
    conn_out: &mut W,
) -> Result<(), E> {
    assert!(!records.is_empty());

    let mut records_iter = records.iter().cloned();
    let mut encoder = encoder::Encoder::new(&mut records_iter, targets, queries, sample_name);

    let bytes = encoder.encode_header_and_flags().unwrap();
    conn_out.write_all(&bytes)?;
    for block in encoder.by_ref() {
        conn_out.write_all(&block)?;
    }

    Ok(())
}

/// Parse all plain-text pseudoalignments from [Read](std::io::Read) and encode to memory.
///
/// ## Usage
/// ```rust
/// use ahda::{encode_from_read, decode_from_read_to_write};
/// use ahda::Format;
/// use std::io::{Cursor, Seek};
///
/// // Mock inputs
/// let targets = vec!["chr.fasta".to_string(), "plasmid.fasta".to_string(), "virus.fasta".to_string()];
/// let queries = vec!["r1".to_string(), "r2".to_string(), "r651903".to_string(), "r7543".to_string(), "r16".to_string()];
/// let name = "sample".to_string();
///
/// // Have this input data:
/// //   3    r7543    chr.fasta:virus.fasta
/// //   0    r1       virus.fasta
/// //   4    r16      chr.fasta:plasmid.fasta:virus.fasta
/// //   2    r651903
/// //
/// let mut input_bytes: Vec<u8> = Vec::new();
/// input_bytes.append(&mut b"0\tr1\tvirus.fasta\n".to_vec());
/// input_bytes.append(&mut b"3\tr7543\tchr.fasta:virus.fasta\n".to_vec());
/// input_bytes.append(&mut b"4\tr16\tchr.fasta:plasmid.fasta:virus.fasta\n".to_vec());
/// input_bytes.append(&mut b"2\tr651903\t\n".to_vec());
///
/// let mut input: Cursor<Vec<u8>> = Cursor::new(input_bytes.clone());
///
/// let output = encode_from_read(&targets, &queries, &name, &mut input).unwrap();
///
/// // `output` can be decoded to get the original data back
/// let mut encoded: Cursor<Vec<u8>> = Cursor::new(output);
/// let mut decoded: Cursor<Vec<u8>> = Cursor::new(Vec::new());
/// decode_from_read_to_write(Format::Metagraph, &mut encoded, &mut decoded).unwrap();
///
/// assert_eq!(decoded.get_ref(), &input_bytes);
/// ```
///
pub fn encode_from_read<R: Read>(
    targets: &[String],
    queries: &[String],
    sample_name: &str,
    conn_in: &mut R,
) -> Result<Vec<u8>, E> {
    let mut reader = crate::parser::Parser::new(conn_in, targets, queries, sample_name)?;
    let mut encoder = encoder::Encoder::new(&mut reader, targets, queries, sample_name);

    let mut bytes = encoder.encode_header_and_flags().unwrap();
    for mut block in encoder.by_ref() {
        bytes.append(&mut block);
    }
    Ok(bytes)
}

/// Parse all plain-text pseudoalignments from [Read](std::io::Read) and encode to [Write](std::io::Write).
///
/// ## Usage
/// ```rust
/// use ahda::{encode_from_read_to_write, decode_from_read_to_write};
/// use ahda::Format;
/// use std::io::{Cursor, Seek};
///
/// // Mock inputs
/// let targets = vec!["chr.fasta".to_string(), "plasmid.fasta".to_string(), "virus.fasta".to_string()];
/// let queries = vec!["r1".to_string(), "r2".to_string(), "r651903".to_string(), "r7543".to_string(), "r16".to_string()];
/// let name = "sample".to_string();
///
/// // Have this input data:
/// //   3    r7543    chr.fasta:virus.fasta
/// //   0    r1       virus.fasta
/// //   4    r16      chr.fasta:plasmid.fasta:virus.fasta
/// //   2    r651903
/// //
/// let mut input_bytes: Vec<u8> = Vec::new();
/// input_bytes.append(&mut b"0\tr1\tvirus.fasta\n".to_vec());
/// input_bytes.append(&mut b"3\tr7543\tchr.fasta:virus.fasta\n".to_vec());
/// input_bytes.append(&mut b"4\tr16\tchr.fasta:plasmid.fasta:virus.fasta\n".to_vec());
/// input_bytes.append(&mut b"2\tr651903\t\n".to_vec());
///
/// let mut input: Cursor<Vec<u8>> = Cursor::new(input_bytes.clone());
///
/// let mut output: Cursor<Vec<u8>> = Cursor::new(Vec::new());
/// encode_from_read_to_write(&targets, &queries, &name, &mut input, &mut output).unwrap();
///
/// // `output` can be decoded to get the original data back
/// output.rewind();
/// let mut decoded: Cursor<Vec<u8>> = Cursor::new(Vec::new());
/// decode_from_read_to_write(Format::Metagraph, &mut output, &mut decoded).unwrap();
///
/// assert_eq!(decoded.get_ref(), &input_bytes);
/// ```
///
pub fn encode_from_read_to_write<R: Read, W: Write>(
    targets: &[String],
    queries: &[String],
    sample_name: &str,
    conn_in: &mut R,
    conn_out: &mut W,
) -> Result<(), E> {
    let mut reader = crate::parser::Parser::new(conn_in, targets, queries, sample_name)?;
    let mut encoder = encoder::Encoder::new(&mut reader, targets, queries, sample_name);

    let bytes = encoder.encode_header_and_flags().unwrap();
    conn_out.write_all(&bytes)?;
    for block in encoder.by_ref() {
        conn_out.write_all(&block)?;
    }
    conn_out.flush()?;
    Ok(())
}

/// Decode all pseudoalignments from [Read](std::io::Read) and format to [Write](std::io::Write).
///
/// ## Usage
/// ```rust
/// use ahda::{decode_from_read_to_write, encode_from_read_to_write};
/// use ahda::Format;
/// use std::io::{Cursor, Seek};
///
/// // Set up mock inputs
/// let targets = vec!["chr.fasta".to_string(), "plasmid.fasta".to_string(), "virus.fasta".to_string()];
/// let queries = vec!["r1".to_string(), "r2".to_string(), "r651903".to_string(), "r7543".to_string(), "r16".to_string()];
/// let name = "sample".to_string();
///
/// let mut plaintext_bytes: Vec<u8> = Vec::new();
/// plaintext_bytes.append(&mut b"0\tr1\tvirus.fasta\n".to_vec());
/// plaintext_bytes.append(&mut b"3\tr7543\tchr.fasta:virus.fasta\n".to_vec());
/// plaintext_bytes.append(&mut b"4\tr16\tchr.fasta:plasmid.fasta:virus.fasta\n".to_vec());
/// plaintext_bytes.append(&mut b"2\tr651903\t\n".to_vec());
///
/// // Encode mock data
/// let mut plaintext: Cursor<Vec<u8>> = Cursor::new(plaintext_bytes.clone());
/// let mut input: Cursor<Vec<u8>> = Cursor::new(Vec::new());
/// encode_from_read_to_write(&targets, &queries, &name, &mut plaintext, &mut input).unwrap();
/// input.rewind();
///
/// // Decode all alignments and compare against the original inputs
/// let mut output: Cursor<Vec<u8>> = Cursor::new(Vec::new());
/// decode_from_read_to_write(Format::Metagraph, &mut input, &mut output).unwrap();
///
/// assert_eq!(output.get_ref(), plaintext.get_ref());
/// ```
///
pub fn decode_from_read_to_write<R: Read, W: Write>(
    out_format: Format,
    conn_in: &mut R,
    conn_out: &mut W,
) -> Result<(), E> {
    let mut decoder = decoder::Decoder::new(conn_in);

    let header = decoder.file_header().clone();
    let flags = decoder.file_flags().clone();
    let printer = printer::Printer::new_from_header_and_flags(&mut decoder, header.clone(), flags.clone(), out_format.clone());
    for line in printer {
        conn_out.write_all(&line)?;
    }

    conn_out.flush()?;
    Ok(())
}

/// Decode all pseudoalignments from [Read](std::io::Read) to memory.
///
/// ## Usage
/// ```rust
/// use ahda::{decode_from_read, encode_to_write};
/// use ahda::PseudoAln;
/// use std::io::{Cursor, Seek};
///
/// // Mock data
/// let data = vec![
///     PseudoAln{ones_names: Some(vec!["chr.fasta".to_string()]),  query_id: Some(0), ones: Some(vec![0]), query_name: Some("ERR4035126.1".to_string()) },
///     PseudoAln{ones_names: Some(vec!["chr.fasta".to_string()]),  query_id: Some(1), ones: Some(vec![0]), query_name: Some("ERR4035126.2".to_string()) },
/// ];
///
/// let targets = vec!["chr.fasta".to_string(), "plasmid.fasta".to_string()];
/// let queries = vec!["ERR4035126.1".to_string(), "ERR4035126.2".to_string(), "ERR4035126.651903".to_string(), "ERR4035126.7543".to_string(), "ERR4035126.16".to_string()];
/// let name = "ERR4035126".to_string();
///
/// // Encode mock data
/// let mut input: Cursor<Vec<u8>> = Cursor::new(Vec::new());
/// encode_to_write(&targets, &queries, &name, &data, &mut input).unwrap();
/// input.rewind();
///
/// // Decode to recover the original data
/// let (_file_header, _file_flags, alns) = decode_from_read(&mut input).unwrap();
///
/// assert_eq!(alns, data);
///
pub fn decode_from_read<R: Read>(
    conn_in: &mut R,
) -> Result<(FileHeader, FileFlags, Vec<PseudoAln>), E> {
    let decoder = decoder::Decoder::new(conn_in);

    let header = decoder.file_header().clone();
    let flags = decoder.file_flags().clone();

    let mut alns: Vec<PseudoAln> = Vec::with_capacity(header.n_queries as usize);
    alns.extend(decoder);

    Ok((header, flags, alns))
}

/// Decode from memory and format to [Write](std::io::Write).
///
/// ## Usage
/// ```rust
/// use ahda::{decode_to_write, encode_to_write};
/// use ahda::{Format, PseudoAln};
/// use std::io::{Cursor, Seek};
///
/// // Mock data
/// let data = vec![
///     PseudoAln{ones_names: Some(vec!["chr.fasta".to_string()]),  query_id: Some(0), ones: Some(vec![0]), query_name: Some("ERR4035126.1".to_string()) },
///     PseudoAln{ones_names: Some(vec!["chr.fasta".to_string()]),  query_id: Some(1), ones: Some(vec![0]), query_name: Some("ERR4035126.2".to_string()) },
/// ];
///
/// let targets = vec!["chr.fasta".to_string(), "plasmid.fasta".to_string()];
/// let queries = vec!["ERR4035126.1".to_string(), "ERR4035126.2".to_string(), "ERR4035126.651903".to_string(), "ERR4035126.7543".to_string(), "ERR4035126.16".to_string()];
/// let name = "ERR4035126".to_string();
///
/// // Encode mock data
/// let mut input: Cursor<Vec<u8>> = Cursor::new(Vec::new());
/// encode_to_write(&targets, &queries, &name, &data, &mut input).unwrap();
///
/// // Decode to recover the original data
/// let mut output: Cursor<Vec<u8>> = Cursor::new(Vec::new());
/// decode_to_write(Format::Metagraph, input.get_ref(), &mut output).unwrap();
///
/// // Expect this output data:
/// //   0    ERR4035126.1    chr.fasta
/// //   1    ERR4035126.2    chr.fasta
/// let mut expected: Vec<u8> = Vec::new();
/// expected.append(&mut b"0\tERR4035126.1\tchr.fasta\n".to_vec());
/// expected.append(&mut b"1\tERR4035126.2\tchr.fasta\n".to_vec());
///
/// assert_eq!(output.get_ref(), &expected);
///
pub fn decode_to_write<W: Write>(
    out_format: Format,
    records: &[u8],
    conn_out: &mut W,
) -> Result<(), E> {
    let mut tmp = std::io::Cursor::new(&records);
    let mut decoder = decoder::Decoder::new(&mut tmp);

    let header = decoder.file_header().clone();
    let flags = decoder.file_flags().clone();
    let printer = printer::Printer::new_from_header_and_flags(&mut decoder, header.clone(), flags.clone(), out_format.clone());
    for line in printer {
        conn_out.write_all(&line)?;
    }

    conn_out.flush()?;
    Ok(())
}

/// Reads the full bitmap and combined block flags from a file
///
/// Returns the [RoaringBitmap] containing all alignments from every block in
/// the input, and the corresponding file header, file flags, and block flags
/// for a single block containing all the data.
///
/// ## Usage
///
/// ```rust
/// use ahda::{decode_from_read_to_roaring, encode_from_read_to_write};
/// use ahda::Format;
/// use ahda::headers::file::{FileHeader, FileFlags};
/// use ahda::headers::block::{BlockHeader, BlockFlags};
/// use roaring::RoaringBitmap;
/// use std::io::{Cursor, Seek};
///
/// // Set up mock inputs
/// let targets = vec!["chr.fasta".to_string(), "plasmid.fasta".to_string(), "virus.fasta".to_string()];
/// let queries = vec!["r1".to_string(), "r2".to_string(), "r651903".to_string(), "r7543".to_string(), "r16".to_string()];
/// let name = "sample".to_string();
///
/// let mut plaintext_bytes: Vec<u8> = Vec::new();
/// plaintext_bytes.append(&mut b"0\tr1\tvirus.fasta\n".to_vec());
/// plaintext_bytes.append(&mut b"3\tr7543\tchr.fasta:virus.fasta\n".to_vec());
/// plaintext_bytes.append(&mut b"4\tr16\tchr.fasta:plasmid.fasta:virus.fasta\n".to_vec());
/// plaintext_bytes.append(&mut b"2\tr651903\t\n".to_vec());
///
/// // Encode mock data
/// let mut plaintext: Cursor<Vec<u8>> = Cursor::new(plaintext_bytes.clone());
/// let mut input: Cursor<Vec<u8>> = Cursor::new(Vec::new());
/// encode_from_read_to_write(&targets, &queries, &name, &mut plaintext, &mut input).unwrap();
/// input.rewind();
///
/// // Decode all alignments and compare against the original inputs
/// let (bitmap, file_header, file_flags, block_flags) = decode_from_read_to_roaring(&mut input).unwrap();
///
/// // Expect these outputs:
/// //   RoaringBitmap<[2, 9, 11, 12, 13, 14]>
/// //   FileHeader   { n_targets: 3, n_queries: 5, flags_len: 44, format: 1, bitmap_type: 0, ph3: 0, ph4: 0 }
/// //   FileFlags    { query_name: "sample", target_names: ["chr.fasta", "plasmid.fasta", "virus.fasta"] }
/// //   BlockFlags   { queries: ["r1", "r651903", "r7543", "r16"], query_ids: [0, 2, 3, 4] }
///
/// assert_eq!(bitmap, RoaringBitmap::from([2, 9, 11, 12, 13, 14]));
/// assert_eq!(file_header, FileHeader{ n_targets: 3, n_queries: 5, flags_len: 44, format: 1, bitmap_type: 0, ph3: 0, ph4: 0 });
/// assert_eq!(file_flags, FileFlags{ query_name: "sample".to_string(), target_names: vec!["chr.fasta".to_string(), "plasmid.fasta".to_string(), "virus.fasta".to_string()] });
/// assert_eq!(block_flags, BlockFlags{ queries: vec!["r1".to_string(), "r651903".to_string(), "r7543".to_string(), "r16".to_string()], query_ids: vec![0, 2, 3, 4] });
///
pub fn decode_from_read_to_roaring<R: Read>(
    conn_in: &mut R,
) -> Result<(RoaringBitmap, FileHeader, FileFlags, BlockFlags), E> {
    let mut bitmap_out = RoaringBitmap::new();
    let header = crate::headers::file::read_file_header(conn_in)?;
    let flags = crate::headers::file::read_file_flags(&header, conn_in)?;

    let mut queries: Vec<String> = Vec::new();
    let mut query_ids: Vec<u32> = Vec::new();

    while let Ok(block_header) = read_block_header(conn_in) {
        let mut block_bytes: Vec<u8> = vec![0; block_header.deflated_len as usize];
        conn_in.read_exact(&mut block_bytes)?;

        let (bitmap, mut block_flags) = unpack_block_roaring32(&block_bytes, &block_header)?;

        queries.append(&mut block_flags.queries);
        query_ids.append(&mut block_flags.query_ids);

        bitmap_out |= bitmap;
    }

    let mut both: Vec<(u32, String)> = queries.iter().zip(query_ids.iter()).map(|(name, idx)| (*idx, name.to_string())).collect();
    both.sort_by_key(|x| x.0);
    let queries: Vec<String> = both.iter().map(|x| x.1.to_string()).collect();
    let query_ids: Vec<u32> = both.iter().map(|x| x.0).collect();

    Ok((bitmap_out, header, flags, BlockFlags{ queries, query_ids }))
}

/// Merge bitmap from Read to an existing bitmap with Union
///
/// Doesn't check that the encoded data was created for compatible data, this
/// just merges the bitmaps.
///
/// ## Usage
///
/// ```rust
/// use ahda::{decode_from_read_into_roaring, decode_from_read_to_roaring, encode_from_read_to_write};
/// use ahda::MergeOp;
/// use roaring::RoaringBitmap;
/// use std::io::{Cursor, Seek};
///
/// // Set up mock inputs
/// let targets = vec!["chr.fasta".to_string(), "plasmid.fasta".to_string(), "virus.fasta".to_string()];
/// let queries = vec!["r1".to_string(), "r2".to_string(), "r651903".to_string(), "r7543".to_string(), "r16".to_string()];
/// let name = "sample".to_string();
///
/// // Have this input data in two files:
/// //     0    r1    virus.fasta
/// //     3    r7543 chr.fasta:virus.fasta
/// //
/// //     0    r1    plasmid.fasta:virus.fasta
/// //     3    r7543 plasmid.fasta
/// //
/// // ...and want to compute the intersection
///
/// let mut plaintext_bytes_1: Vec<u8> = Vec::new();
/// plaintext_bytes_1.append(&mut b"0\tr1\tvirus.fasta\n".to_vec());
/// plaintext_bytes_1.append(&mut b"3\tr7543\tchr.fasta:virus.fasta\n".to_vec());
///
/// let mut plaintext_bytes_2: Vec<u8> = Vec::new();
/// plaintext_bytes_2.append(&mut b"0\tr1\tplasmid.fasta:virus.fasta\n".to_vec());
/// plaintext_bytes_2.append(&mut b"3\tr7543\tplasmid.fasta\n".to_vec());
///
/// // Encode mock data
/// let mut plaintext_1: Cursor<Vec<u8>> = Cursor::new(plaintext_bytes_1.clone());
/// let mut input_1: Cursor<Vec<u8>> = Cursor::new(Vec::new());
/// encode_from_read_to_write(&targets, &queries, &name, &mut plaintext_1, &mut input_1).unwrap();
/// input_1.rewind();
///
/// let mut plaintext_2: Cursor<Vec<u8>> = Cursor::new(plaintext_bytes_2.clone());
/// let mut input_2: Cursor<Vec<u8>> = Cursor::new(Vec::new());
/// encode_from_read_to_write(&targets, &queries, &name, &mut plaintext_2, &mut input_2).unwrap();
/// input_2.rewind();
///
/// // Decode data from `input_1`
/// let (mut bitmap, _file_header, _file_flags, _block_flags) = decode_from_read_to_roaring(&mut input_1).unwrap();
///
/// // Intersect data from `input_2` with the decoded `bitmap`
/// decode_from_read_into_roaring(&mut input_2, &MergeOp::Intersection, &mut bitmap).unwrap();
///
/// // Expect these outputs:
/// //     RoaringBitmap<[2]>
/// // ...ie the alignment in the intersection is:
/// //     0    r1    virus.fasta
///
/// assert_eq!(bitmap, RoaringBitmap::from([2]));
///
pub fn decode_from_read_into_roaring<R: Read>(
    conn_in: &mut R,
    merge_op: &MergeOp,
    bitmap_out: &mut RoaringBitmap,
) -> Result<(), E> {
    match merge_op {
        MergeOp::Intersection => {
            // Have to read in the whole bitmap to perform intersection
            let (bitmap_b, _, _, _) = decode_from_read_to_roaring(conn_in)?;
            *bitmap_out &= bitmap_b;
        },
        _ => {
            let header = crate::headers::file::read_file_header(conn_in)?;
            let _ = crate::headers::file::read_file_flags(&header, conn_in)?;

            while let Ok(block_header) = read_block_header(conn_in) {
                let mut block_bytes: Vec<u8> = vec![0; block_header.deflated_len as usize];
                conn_in.read_exact(&mut block_bytes)?;

                let (bitmap_b, _) = unpack_block_roaring32(&block_bytes, &block_header)?;

                match merge_op {
                    MergeOp::Union => {
                        *bitmap_out |= bitmap_b;
                    },
                    MergeOp::Xor => {
                        *bitmap_out ^= bitmap_b;
                    },
                    MergeOp::Diff => {
                        *bitmap_out -= bitmap_b;
                    },
                    _ => panic!("This shouldn't happen"),
                }
            }
        },
    }

    Ok(())
}

#[cfg(test)]
mod tests {

    #[test]
    fn concatenate_from_read_to_write() {
        use super::concatenate_from_read_to_write;

        use std::io::Cursor;

        let data_bytes_1: Vec<u8> = vec![2, 0, 0, 0, 5, 0, 0, 0, 36, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 10, 69, 82, 82, 52, 48, 51, 53, 49, 50, 54, 2, 9, 99, 104, 114, 46, 102, 97, 115, 116, 97, 13, 112, 108, 97, 115, 109, 105, 100, 46, 102, 97, 115, 116, 97, 2, 0, 0, 0, 72, 0, 0, 0, 20, 0, 0, 0, 30, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 147, 239, 230, 96, 0, 131, 255, 155, 141, 18, 18, 18, 82, 24, 24, 221, 216, 24, 13, 206, 30, 57, 112, 228, 177, 148, 72, 74, 82, 66, 78, 86, 70, 202, 178, 244, 142, 51, 134, 73, 73, 9, 44, 12, 166, 66, 39, 86, 27, 49, 48, 48, 0, 0, 86, 244, 9, 212, 54, 0, 0, 0];
        let data_bytes_2: Vec<u8> = vec![2, 0, 0, 0, 5, 0, 0, 0, 36, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 10, 69, 82, 82, 52, 48, 51, 53, 49, 50, 54, 2, 9, 99, 104, 114, 46, 102, 97, 115, 116, 97, 13, 112, 108, 97, 115, 109, 105, 100, 46, 102, 97, 115, 116, 97, 1, 0, 0, 0, 70, 0, 0, 0, 20, 0, 0, 0, 21, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 147, 239, 230, 96, 0, 131, 255, 155, 141, 18, 18, 18, 82, 24, 24, 221, 216, 24, 53, 206, 106, 188, 144, 18, 73, 73, 74, 200, 201, 202, 72, 89, 150, 158, 150, 149, 145, 153, 112, 230, 4, 11, 195, 3, 205, 69, 179, 53, 25, 24, 24, 0, 14, 31, 76, 77, 53, 0, 0, 0];
        let data_bytes_3: Vec<u8> = vec![2, 0, 0, 0, 5, 0, 0, 0, 36, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 10, 69, 82, 82, 52, 48, 51, 53, 49, 50, 54, 2, 9, 99, 104, 114, 46, 102, 97, 115, 116, 97, 13, 112, 108, 97, 115, 109, 105, 100, 46, 102, 97, 115, 116, 97, 2, 0, 0, 0, 75, 0, 0, 0, 18, 0, 0, 0, 34, 0, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 147, 239, 230, 96, 0, 131, 255, 155, 141, 18, 18, 18, 82, 26, 24, 24, 217, 4, 102, 119, 222, 55, 213, 56, 113, 228, 192, 141, 43, 23, 78, 248, 156, 191, 115, 229, 200, 12, 241, 206, 252, 140, 99, 71, 206, 48, 49, 148, 77, 218, 158, 105, 194, 192, 192, 0, 0, 28, 8, 109, 15, 56, 0, 0, 0];
        let data_1 = Cursor::new(data_bytes_1);
        let data_2 = Cursor::new(data_bytes_2);
        let data_3 = Cursor::new(data_bytes_3);
        let mut data = vec![data_1, data_2, data_3];

        let expected: Vec<u8> = vec![2, 0, 0, 0, 15, 0, 0, 0, 36, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 10, 69, 82, 82, 52, 48, 51, 53, 49, 50, 54, 2, 9, 99, 104, 114, 46, 102, 97, 115, 116, 97, 13, 112, 108, 97, 115, 109, 105, 100, 46, 102, 97, 115, 116, 97, 2, 0, 0, 0, 72, 0, 0, 0, 20, 0, 0, 0, 30, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 147, 239, 230, 96, 0, 131, 255, 155, 141, 18, 18, 18, 82, 24, 24, 221, 216, 24, 13, 206, 30, 57, 112, 228, 177, 148, 72, 74, 82, 66, 78, 86, 70, 202, 178, 244, 142, 51, 134, 73, 73, 9, 44, 12, 166, 66, 39, 86, 27, 49, 48, 48, 0, 0, 86, 244, 9, 212, 54, 0, 0, 0, 1, 0, 0, 0, 70, 0, 0, 0, 20, 0, 0, 0, 21, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 147, 239, 230, 96, 0, 131, 255, 155, 141, 18, 18, 18, 82, 24, 24, 221, 216, 24, 53, 206, 106, 188, 144, 18, 73, 73, 74, 200, 201, 202, 72, 89, 150, 158, 150, 149, 145, 153, 112, 230, 4, 11, 195, 3, 205, 69, 179, 53, 25, 24, 24, 0, 14, 31, 76, 77, 53, 0, 0, 0, 2, 0, 0, 0, 75, 0, 0, 0, 18, 0, 0, 0, 34, 0, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 147, 239, 230, 96, 0, 131, 255, 155, 141, 18, 18, 18, 82, 26, 24, 24, 217, 4, 102, 119, 222, 55, 213, 56, 113, 228, 192, 141, 43, 23, 78, 248, 156, 191, 115, 229, 200, 12, 241, 206, 252, 140, 99, 71, 206, 48, 49, 148, 77, 218, 158, 105, 194, 192, 192, 0, 0, 28, 8, 109, 15, 56, 0, 0, 0];

        let mut bytes_got: Cursor<Vec<u8>> = Cursor::new(Vec::new());
        concatenate_from_read_to_write(&mut data, &mut bytes_got).unwrap();

        let got = bytes_got.get_ref();

        assert_eq!(*got, expected);
    }

    #[test]
    fn convert_from_read_to_write() {
        use super::convert_from_read_to_write;

        use crate::Format;

        use std::io::Cursor;

        let data_bytes: Vec<u8> = vec![49, 9, 69, 82, 82, 52, 48, 51, 53, 49, 50, 54, 46, 50, 9, 99, 104, 114, 46, 102, 97, 115, 116, 97, 10, 48, 9, 69, 82, 82, 52, 48, 51, 53, 49, 50, 54, 46, 49, 9, 99, 104, 114, 46, 102, 97, 115, 116, 97, 10, 50, 9, 69, 82, 82, 52, 48, 51, 53, 49, 50, 54, 46, 54, 53, 49, 57, 48, 51, 9, 99, 104, 114, 46, 102, 97, 115, 116, 97, 58, 112, 108, 97, 115, 109, 105, 100, 46, 102, 97, 115, 116, 97, 10, 52, 9, 69, 82, 82, 52, 48, 51, 53, 49, 50, 54, 46, 49, 54, 9, 10, 51, 9, 69, 82, 82, 52, 48, 51, 53, 49, 50, 54, 46, 55, 53, 52, 51, 9, 112, 108, 97, 115, 109, 105, 100, 46, 102, 97, 115, 116, 97, 10];
        let mut data = Cursor::new(data_bytes);

        let expected = b"query_name\tchr.fasta\tplasmid.fasta\nERR4035126.2\t1\t0\nERR4035126.1\t1\t0\nERR4035126.651903\t1\t1\nERR4035126.16\t0\t0\nERR4035126.7543\t0\t1\n".to_vec();

        let targets = vec!["chr.fasta".to_string(), "plasmid.fasta".to_string()];
        let queries = vec!["ERR4035126.1".to_string(), "ERR4035126.2".to_string(), "ERR4035126.651903".to_string(), "ERR4035126.7543".to_string(), "ERR4035126.16".to_string()];
        let query_name ="ERR4035126".to_string();

        let out_format = Format::Bifrost;

        let mut bytes_got: Cursor<Vec<u8>> = Cursor::new(Vec::new());
        convert_from_read_to_write(&targets, &queries, &query_name, out_format, &mut data, &mut bytes_got).unwrap();
        let got = bytes_got.get_ref();

        assert_eq!(*got, expected);
    }

    #[test]
    fn encode_to_write() {
        use super::encode_to_write;

        use crate::PseudoAln;

        use std::io::Cursor;

        let data = vec![
            PseudoAln{ones_names: Some(vec!["chr.fasta".to_string()]),  query_id: Some(1), ones: Some(vec![0]), query_name: Some("ERR4035126.2".to_string()) },
            PseudoAln{ones_names: Some(vec!["chr.fasta".to_string()]),  query_id: Some(0), ones: Some(vec![0]), query_name: Some("ERR4035126.1".to_string()) },
            PseudoAln{ones_names: Some(vec!["chr.fasta".to_string(), "plasmid.fasta".to_string()]),  query_id: Some(2), ones: Some(vec![0, 1]), query_name: Some("ERR4035126.651903".to_string()) },
            PseudoAln{ones_names: Some(vec![]),  query_id: Some(4), ones: Some(vec![]), query_name: Some("ERR4035126.16".to_string()) },
            PseudoAln{ones_names: Some(vec!["plasmid.fasta".to_string()]),  query_id: Some(3), ones: Some(vec![1]), query_name: Some("ERR4035126.7543".to_string()) },
        ];
        let mut bytes: Cursor<Vec<u8>> = Cursor::new(Vec::new());

        let targets = vec!["chr.fasta".to_string(), "plasmid.fasta".to_string()];
        let queries = vec!["ERR4035126.1".to_string(), "ERR4035126.2".to_string(), "ERR4035126.651903".to_string(), "ERR4035126.7543".to_string(), "ERR4035126.16".to_string()];
        let sample = "ERR4035126".to_string();

        encode_to_write(&targets, &queries, &sample, &data, &mut bytes).unwrap();

        let expected: Vec<u8> = vec![2, 0, 0, 0, 5, 0, 0, 0, 36, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 10, 69, 82, 82, 52, 48, 51, 53, 49, 50, 54, 2, 9, 99, 104, 114, 46, 102, 97, 115, 116, 97, 13, 112, 108, 97, 115, 109, 105, 100, 46, 102, 97, 115, 116, 97, 5, 0, 0, 0, 103, 0, 0, 0, 40, 0, 0, 0, 63, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 99, 229, 113, 13, 10, 50, 49, 48, 54, 53, 52, 50, 211, 51, 68, 230, 24, 9, 34, 113, 204, 76, 13, 45, 13, 140, 249, 145, 68, 204, 77, 77, 140, 121, 145, 245, 154, 177, 50, 48, 50, 49, 179, 0, 0, 164, 198, 115, 218, 81, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 179, 50, 96, 96, 96, 100, 0, 1, 22, 6, 1, 48, 205, 196, 192, 194, 192, 202, 192, 206, 0, 0, 47, 109, 177, 38, 26, 0, 0, 0];

        assert_eq!(*bytes.get_ref(), expected);
    }

    #[test]
    fn encode_from_read() {
        use super::encode_from_read;

        use super::headers::file::build_header_and_flags;

        use crate::PseudoAln;
        use crate::Format;

        use std::io::Cursor;
        use std::io::Seek;
        use std::io::Write;

        let data = vec![
            PseudoAln{ones_names: Some(vec!["chr.fasta".to_string()]),  query_id: Some(1), ones: Some(vec![0]), query_name: Some("ERR4035126.2".to_string()) },
            PseudoAln{ones_names: Some(vec!["chr.fasta".to_string()]),  query_id: Some(0), ones: Some(vec![0]), query_name: Some("ERR4035126.1".to_string()) },
            PseudoAln{ones_names: Some(vec!["chr.fasta".to_string(), "plasmid.fasta".to_string()]),  query_id: Some(2), ones: Some(vec![0, 1]), query_name: Some("ERR4035126.651903".to_string()) },
            PseudoAln{ones_names: Some(vec![]),  query_id: Some(4), ones: Some(vec![]), query_name: Some("ERR4035126.16".to_string()) },
            PseudoAln{ones_names: Some(vec!["plasmid.fasta".to_string()]),  query_id: Some(3), ones: Some(vec![1]), query_name: Some("ERR4035126.7543".to_string()) },
        ];

        let mut bytes: Cursor<Vec<u8>> = Cursor::new(Vec::new());

        let expected: Vec<u8> = vec![2, 0, 0, 0, 5, 0, 0, 0, 36, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 10, 69, 82, 82, 52, 48, 51, 53, 49, 50, 54, 2, 9, 99, 104, 114, 46, 102, 97, 115, 116, 97, 13, 112, 108, 97, 115, 109, 105, 100, 46, 102, 97, 115, 116, 97, 5, 0, 0, 0, 103, 0, 0, 0, 40, 0, 0, 0, 63, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 99, 229, 113, 13, 10, 50, 49, 48, 54, 53, 52, 50, 211, 51, 68, 230, 24, 9, 34, 113, 204, 76, 13, 45, 13, 140, 249, 145, 68, 204, 77, 77, 140, 121, 145, 245, 154, 177, 50, 48, 50, 49, 179, 0, 0, 164, 198, 115, 218, 81, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 179, 50, 96, 96, 96, 100, 0, 1, 22, 6, 1, 48, 205, 196, 192, 194, 192, 202, 192, 206, 0, 0, 47, 109, 177, 38, 26, 0, 0, 0];

        let targets = vec!["chr.fasta".to_string(), "plasmid.fasta".to_string()];
        let queries = vec!["ERR4035126.1".to_string(), "ERR4035126.2".to_string(), "ERR4035126.651903".to_string(), "ERR4035126.7543".to_string(), "ERR4035126.16".to_string()];
        let query_name ="ERR4035126".to_string();

        let (header, flags) = build_header_and_flags(&targets, &queries, &query_name).unwrap();
        let format = Format::Metagraph;

        let mut tmp = data.into_iter();
        let mut writer = crate::printer::Printer::new_from_header_and_flags(&mut tmp, header, flags, format);
        for record in writer.by_ref() {
            bytes.write(&record).unwrap();
        }
        bytes.rewind().unwrap();

        let got = encode_from_read(&targets, &queries, &query_name, &mut bytes).unwrap();

        assert_eq!(got, expected);
    }

    #[test]
    fn encode_from_read_to_write() {
        use super::encode_from_read_to_write;

        use std::io::Cursor;

        let data_bytes: Vec<u8> = vec![49, 9, 69, 82, 82, 52, 48, 51, 53, 49, 50, 54, 46, 50, 9, 99, 104, 114, 46, 102, 97, 115, 116, 97, 10, 48, 9, 69, 82, 82, 52, 48, 51, 53, 49, 50, 54, 46, 49, 9, 99, 104, 114, 46, 102, 97, 115, 116, 97, 10, 50, 9, 69, 82, 82, 52, 48, 51, 53, 49, 50, 54, 46, 54, 53, 49, 57, 48, 51, 9, 99, 104, 114, 46, 102, 97, 115, 116, 97, 58, 112, 108, 97, 115, 109, 105, 100, 46, 102, 97, 115, 116, 97, 10, 52, 9, 69, 82, 82, 52, 48, 51, 53, 49, 50, 54, 46, 49, 54, 9, 10, 51, 9, 69, 82, 82, 52, 48, 51, 53, 49, 50, 54, 46, 55, 53, 52, 51, 9, 112, 108, 97, 115, 109, 105, 100, 46, 102, 97, 115, 116, 97, 10];
        let mut data = Cursor::new(data_bytes);

        let expected: Vec<u8> = vec![2, 0, 0, 0, 5, 0, 0, 0, 36, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 10, 69, 82, 82, 52, 48, 51, 53, 49, 50, 54, 2, 9, 99, 104, 114, 46, 102, 97, 115, 116, 97, 13, 112, 108, 97, 115, 109, 105, 100, 46, 102, 97, 115, 116, 97, 5, 0, 0, 0, 103, 0, 0, 0, 40, 0, 0, 0, 63, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 99, 229, 113, 13, 10, 50, 49, 48, 54, 53, 52, 50, 211, 51, 68, 230, 24, 9, 34, 113, 204, 76, 13, 45, 13, 140, 249, 145, 68, 204, 77, 77, 140, 121, 145, 245, 154, 177, 50, 48, 50, 49, 179, 0, 0, 164, 198, 115, 218, 81, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 179, 50, 96, 96, 96, 100, 0, 1, 22, 6, 1, 48, 205, 196, 192, 194, 192, 202, 192, 206, 0, 0, 47, 109, 177, 38, 26, 0, 0, 0];

        let targets = vec!["chr.fasta".to_string(), "plasmid.fasta".to_string()];
        let queries = vec!["ERR4035126.1".to_string(), "ERR4035126.2".to_string(), "ERR4035126.651903".to_string(), "ERR4035126.7543".to_string(), "ERR4035126.16".to_string()];
        let query_name ="ERR4035126".to_string();

        let mut bytes_got: Cursor<Vec<u8>> = Cursor::new(Vec::new());
        encode_from_read_to_write(&targets, &queries, &query_name, &mut data, &mut bytes_got).unwrap();
        let got = bytes_got.get_ref();

        assert_eq!(*got, expected);
    }

    #[test]
    fn decode_from_read() {
        use super::decode_from_read;
        use super::headers::file::build_header_and_flags;
        use crate::PseudoAln;

        use std::io::Cursor;

        let mut expected_alns = vec![
            PseudoAln{ones_names: Some(vec!["chr.fasta".to_string()]),  query_id: Some(1), ones: Some(vec![0]), query_name: Some("ERR4035126.2".to_string()) },
            PseudoAln{ones_names: Some(vec!["chr.fasta".to_string()]),  query_id: Some(0), ones: Some(vec![0]), query_name: Some("ERR4035126.1".to_string()) },
            PseudoAln{ones_names: Some(vec!["chr.fasta".to_string(), "plasmid.fasta".to_string()]),  query_id: Some(2), ones: Some(vec![0, 1]), query_name: Some("ERR4035126.651903".to_string()) },
            PseudoAln{ones_names: Some(vec![]),  query_id: Some(4), ones: Some(vec![]), query_name: Some("ERR4035126.16".to_string()) },
            PseudoAln{ones_names: Some(vec!["plasmid.fasta".to_string()]),  query_id: Some(3), ones: Some(vec![1]), query_name: Some("ERR4035126.7543".to_string()) },
        ];
        expected_alns.sort_by_key(|x| *x.query_id.as_ref().unwrap());
        let (expected_header, expected_flags) = build_header_and_flags(&vec!["chr.fasta".to_string(), "plasmid.fasta".to_string()], &vec!["ERR4035126.1".to_string(), "ERR4035126.2".to_string(), "ERR4035126.651903".to_string(), "ERR4035126.7543".to_string(), "ERR4035126.16".to_string()], &"ERR4035126".to_string()).unwrap();

        let data: Vec<u8> = vec![2, 0, 0, 0, 5, 0, 0, 0, 36, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 10, 69, 82, 82, 52, 48, 51, 53, 49, 50, 54, 2, 9, 99, 104, 114, 46, 102, 97, 115, 116, 97, 13, 112, 108, 97, 115, 109, 105, 100, 46, 102, 97, 115, 116, 97, 5, 0, 0, 0, 103, 0, 0, 0, 40, 0, 0, 0, 63, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 99, 229, 113, 13, 10, 50, 49, 48, 54, 53, 52, 50, 211, 51, 68, 230, 24, 9, 34, 113, 204, 76, 13, 45, 13, 140, 249, 145, 68, 204, 77, 77, 140, 121, 145, 245, 154, 177, 50, 48, 50, 49, 179, 0, 0, 164, 198, 115, 218, 81, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 179, 50, 96, 96, 96, 100, 0, 1, 22, 6, 1, 48, 205, 196, 192, 194, 192, 202, 192, 206, 0, 0, 47, 109, 177, 38, 26, 0, 0, 0];
        let mut bytes: Cursor<Vec<u8>> = Cursor::new(data);

        let (file_header, file_flags, mut got) = decode_from_read(&mut bytes).unwrap();
        got.sort_by_key(|x| *x.query_id.as_ref().unwrap());

        assert_eq!(expected_header, file_header);
        assert_eq!(expected_flags, file_flags);
        assert_eq!(expected_alns, got);
    }

    #[test]
    fn decode_to_write() {
        use super::decode_to_write;
        use crate::Format;

        use std::io::Cursor;

        let expected = b"0\tERR4035126.1\tchr.fasta\n1\tERR4035126.2\tchr.fasta\n2\tERR4035126.651903\tchr.fasta:plasmid.fasta\n3\tERR4035126.7543\tplasmid.fasta\n4\tERR4035126.16\t\n";
        let data: Vec<u8> = vec![2, 0, 0, 0, 5, 0, 0, 0, 36, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 10, 69, 82, 82, 52, 48, 51, 53, 49, 50, 54, 2, 9, 99, 104, 114, 46, 102, 97, 115, 116, 97, 13, 112, 108, 97, 115, 109, 105, 100, 46, 102, 97, 115, 116, 97, 5, 0, 0, 0, 103, 0, 0, 0, 40, 0, 0, 0, 63, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 99, 229, 113, 13, 10, 50, 49, 48, 54, 53, 52, 50, 211, 51, 68, 230, 24, 9, 34, 113, 204, 76, 13, 45, 13, 140, 249, 145, 68, 204, 77, 77, 140, 121, 145, 245, 154, 177, 50, 48, 50, 49, 179, 0, 0, 164, 198, 115, 218, 81, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 179, 50, 96, 96, 96, 100, 0, 1, 22, 6, 1, 48, 205, 196, 192, 194, 192, 202, 192, 206, 0, 0, 47, 109, 177, 38, 26, 0, 0, 0];
        let format = Format::Metagraph;

        let mut got_bytes: Cursor<Vec<u8>> = Cursor::new(Vec::new());
        decode_to_write(format, &data, &mut got_bytes).unwrap();

        let got = got_bytes.get_ref();

        assert_eq!(*got, *expected);
    }

    #[test]
    fn decode_from_read_to_write() {
        use super::decode_from_read_to_write;
        use crate::Format;

        use std::io::Cursor;

        let expected = b"0\tERR4035126.1\tchr.fasta\n1\tERR4035126.2\tchr.fasta\n2\tERR4035126.651903\tchr.fasta:plasmid.fasta\n3\tERR4035126.7543\tplasmid.fasta\n4\tERR4035126.16\t\n";
        let data_bytes: Vec<u8> = vec![2, 0, 0, 0, 5, 0, 0, 0, 36, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 10, 69, 82, 82, 52, 48, 51, 53, 49, 50, 54, 2, 9, 99, 104, 114, 46, 102, 97, 115, 116, 97, 13, 112, 108, 97, 115, 109, 105, 100, 46, 102, 97, 115, 116, 97, 5, 0, 0, 0, 103, 0, 0, 0, 40, 0, 0, 0, 63, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 99, 229, 113, 13, 10, 50, 49, 48, 54, 53, 52, 50, 211, 51, 68, 230, 24, 9, 34, 113, 204, 76, 13, 45, 13, 140, 249, 145, 68, 204, 77, 77, 140, 121, 145, 245, 154, 177, 50, 48, 50, 49, 179, 0, 0, 164, 198, 115, 218, 81, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 179, 50, 96, 96, 96, 100, 0, 1, 22, 6, 1, 48, 205, 196, 192, 194, 192, 202, 192, 206, 0, 0, 47, 109, 177, 38, 26, 0, 0, 0];
        let mut data: Cursor<Vec<u8>> = Cursor::new(data_bytes);
        let format = Format::Metagraph;

        let mut got_bytes: Cursor<Vec<u8>> = Cursor::new(Vec::new());
        decode_from_read_to_write(format, &mut data, &mut got_bytes).unwrap();

        let got = got_bytes.get_ref();

        eprintln!("{:?}", got.iter().map(|x| *x as char).collect::<String>());
        eprintln!("{:?}", expected.iter().map(|x| *x as char).collect::<String>());
        assert_eq!(*got, *expected);
    }

    #[test]
    fn decode_from_read_to_roaring() {
        use super::decode_from_read_to_roaring;
        use super::headers::file::build_header_and_flags;
        use super::headers::block::BlockFlags;

        use std::io::Cursor;

        use roaring::RoaringBitmap;

        let mut expected = RoaringBitmap::new();
        expected.insert(0);
        expected.insert(2);
        expected.insert(4);
        expected.insert(5);
        expected.insert(7);

        let targets = vec!["chr.fasta".to_string(), "plasmid.fasta".to_string()];
        let queries = vec!["ERR4035126.1".to_string(), "ERR4035126.2".to_string(), "ERR4035126.651903".to_string(), "ERR4035126.7543".to_string(), "ERR4035126.16".to_string()];
        let query_ids = vec![0, 1, 2, 3, 4];
        let expected_block_flags = BlockFlags { queries: queries.clone(), query_ids };
        let (expected_header, expected_flags) = build_header_and_flags(&targets, &queries, &"ERR4035126".to_string()).unwrap();

        let data: Vec<u8> = vec![2, 0, 0, 0, 5, 0, 0, 0, 36, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 10, 69, 82, 82, 52, 48, 51, 53, 49, 50, 54, 2, 9, 99, 104, 114, 46, 102, 97, 115, 116, 97, 13, 112, 108, 97, 115, 109, 105, 100, 46, 102, 97, 115, 116, 97, 5, 0, 0, 0, 103, 0, 0, 0, 40, 0, 0, 0, 63, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 99, 229, 113, 13, 10, 50, 49, 48, 54, 53, 52, 50, 211, 51, 68, 230, 24, 9, 34, 113, 204, 76, 13, 45, 13, 140, 249, 145, 68, 204, 77, 77, 140, 121, 145, 245, 154, 177, 50, 48, 50, 49, 179, 0, 0, 164, 198, 115, 218, 81, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 179, 50, 96, 96, 96, 100, 0, 1, 22, 6, 1, 48, 205, 196, 192, 194, 192, 202, 192, 206, 0, 0, 47, 109, 177, 38, 26, 0, 0, 0];
        let mut bytes: Cursor<Vec<u8>> = Cursor::new(data);

        let (got, file_header, file_flags, block_flags) = decode_from_read_to_roaring(&mut bytes).unwrap();

        assert_eq!(expected_header, file_header);
        assert_eq!(expected_flags, file_flags);
        assert_eq!(expected_block_flags, block_flags);
        assert_eq!(got, expected);
    }

    #[test]
    fn decode_from_read_into_roaring_union() {
        use super::decode_from_read_into_roaring;
        use super::MergeOp;

        use std::io::Cursor;

        use roaring::RoaringBitmap;

        let data_right: Vec<u8> = vec![2, 0, 0, 0, 5, 0, 0, 0, 36, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 10, 69, 82, 82, 52, 48, 51, 53, 49, 50, 54, 2, 9, 99, 104, 114, 46, 102, 97, 115, 116, 97, 13, 112, 108, 97, 115, 109, 105, 100, 46, 102, 97, 115, 116, 97, 5, 0, 0, 0, 103, 0, 0, 0, 40, 0, 0, 0, 63, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 99, 229, 113, 13, 10, 50, 49, 48, 54, 53, 52, 50, 211, 51, 68, 230, 24, 9, 34, 113, 204, 76, 13, 45, 13, 140, 249, 145, 68, 204, 77, 77, 140, 121, 145, 245, 154, 177, 50, 48, 50, 49, 179, 0, 0, 164, 198, 115, 218, 81, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 179, 50, 96, 96, 96, 100, 0, 1, 22, 6, 1, 48, 205, 196, 192, 194, 192, 202, 192, 206, 0, 0, 47, 109, 177, 38, 26, 0, 0, 0];
        let mut data: Cursor<Vec<u8>> = Cursor::new(data_right);

        let mut data_left = RoaringBitmap::new();
        data_left.insert(1);
        data_left.insert(5);
        data_left.insert(8);

        let mut expected = RoaringBitmap::new();
        expected.insert(0);
        expected.insert(1);
        expected.insert(2);
        expected.insert(4);
        expected.insert(5);
        expected.insert(7);
        expected.insert(8);

        decode_from_read_into_roaring(&mut data, &MergeOp::Union, &mut data_left).unwrap();

        assert_eq!(data_left, expected);
    }

    #[test]
    fn decode_from_read_into_roaring_intersection() {
        use super::decode_from_read_into_roaring;
        use super::MergeOp;

        use std::io::Cursor;

        use roaring::RoaringBitmap;

        let data_right: Vec<u8> = vec![2, 0, 0, 0, 5, 0, 0, 0, 36, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 10, 69, 82, 82, 52, 48, 51, 53, 49, 50, 54, 2, 9, 99, 104, 114, 46, 102, 97, 115, 116, 97, 13, 112, 108, 97, 115, 109, 105, 100, 46, 102, 97, 115, 116, 97, 5, 0, 0, 0, 103, 0, 0, 0, 40, 0, 0, 0, 63, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 99, 229, 113, 13, 10, 50, 49, 48, 54, 53, 52, 50, 211, 51, 68, 230, 24, 9, 34, 113, 204, 76, 13, 45, 13, 140, 249, 145, 68, 204, 77, 77, 140, 121, 145, 245, 154, 177, 50, 48, 50, 49, 179, 0, 0, 164, 198, 115, 218, 81, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 179, 50, 96, 96, 96, 100, 0, 1, 22, 6, 1, 48, 205, 196, 192, 194, 192, 202, 192, 206, 0, 0, 47, 109, 177, 38, 26, 0, 0, 0];
        let mut data: Cursor<Vec<u8>> = Cursor::new(data_right);

        let mut data_left = RoaringBitmap::new();
        data_left.insert(0);
        data_left.insert(3);
        data_left.insert(7);

        let mut expected = RoaringBitmap::new();
        expected.insert(0);
        expected.insert(7);

        decode_from_read_into_roaring(&mut data, &MergeOp::Intersection, &mut data_left).unwrap();

        assert_eq!(data_left, expected);
    }

    #[test]
    fn decode_from_read_into_roaring_xor() {
        use super::decode_from_read_into_roaring;
        use super::MergeOp;

        use std::io::Cursor;

        use roaring::RoaringBitmap;

        let data_right: Vec<u8> = vec![2, 0, 0, 0, 5, 0, 0, 0, 36, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 10, 69, 82, 82, 52, 48, 51, 53, 49, 50, 54, 2, 9, 99, 104, 114, 46, 102, 97, 115, 116, 97, 13, 112, 108, 97, 115, 109, 105, 100, 46, 102, 97, 115, 116, 97, 5, 0, 0, 0, 103, 0, 0, 0, 40, 0, 0, 0, 63, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 99, 229, 113, 13, 10, 50, 49, 48, 54, 53, 52, 50, 211, 51, 68, 230, 24, 9, 34, 113, 204, 76, 13, 45, 13, 140, 249, 145, 68, 204, 77, 77, 140, 121, 145, 245, 154, 177, 50, 48, 50, 49, 179, 0, 0, 164, 198, 115, 218, 81, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 179, 50, 96, 96, 96, 100, 0, 1, 22, 6, 1, 48, 205, 196, 192, 194, 192, 202, 192, 206, 0, 0, 47, 109, 177, 38, 26, 0, 0, 0];
        let mut data: Cursor<Vec<u8>> = Cursor::new(data_right);

        let mut data_left = RoaringBitmap::new();
        data_left.insert(0);
        data_left.insert(1);
        data_left.insert(5);
        data_left.insert(7);

        let mut expected = RoaringBitmap::new();
        expected.insert(1);
        expected.insert(2);
        expected.insert(4);

        decode_from_read_into_roaring(&mut data, &MergeOp::Xor, &mut data_left).unwrap();

        assert_eq!(data_left, expected);
    }

    #[test]
    fn decode_from_read_into_roaring_diff() {
        use super::decode_from_read_into_roaring;
        use super::MergeOp;

        use std::io::Cursor;

        use roaring::RoaringBitmap;

        let data_right: Vec<u8> = vec![2, 0, 0, 0, 5, 0, 0, 0, 36, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 10, 69, 82, 82, 52, 48, 51, 53, 49, 50, 54, 2, 9, 99, 104, 114, 46, 102, 97, 115, 116, 97, 13, 112, 108, 97, 115, 109, 105, 100, 46, 102, 97, 115, 116, 97, 5, 0, 0, 0, 103, 0, 0, 0, 40, 0, 0, 0, 63, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 99, 229, 113, 13, 10, 50, 49, 48, 54, 53, 52, 50, 211, 51, 68, 230, 24, 9, 34, 113, 204, 76, 13, 45, 13, 140, 249, 145, 68, 204, 77, 77, 140, 121, 145, 245, 154, 177, 50, 48, 50, 49, 179, 0, 0, 164, 198, 115, 218, 81, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 179, 50, 96, 96, 96, 100, 0, 1, 22, 6, 1, 48, 205, 196, 192, 194, 192, 202, 192, 206, 0, 0, 47, 109, 177, 38, 26, 0, 0, 0];
        let mut data: Cursor<Vec<u8>> = Cursor::new(data_right);

        let mut data_left = RoaringBitmap::new();
        data_left.insert(0);
        data_left.insert(2);
        data_left.insert(3);
        data_left.insert(4);
        data_left.insert(5);
        data_left.insert(6);
        data_left.insert(7);

        let mut expected = RoaringBitmap::new();
        expected.insert(3);
        expected.insert(6);

        decode_from_read_into_roaring(&mut data, &MergeOp::Diff, &mut data_left).unwrap();

        assert_eq!(data_left, expected);
    }
}
