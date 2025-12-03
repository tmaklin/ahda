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

//! ahda is a library and a command-line client for converting between
//! pseudoalignment formats output by different tools and for compressing the
//! data by up to 1000x compared to plaintext and 100x compared to gzip.
//!
//! ahda supports the following three operations:
//!   - `ahda cat` print input file(s) in another format.
//!   - `ahda decode` decompress pseudoalignment data to a supported format.
//!   - `ahda encode` compress pseudoalignment data from a supported format.
//!
//! ahda can read input data from the following formats:
//!   - [Bifrost](https://github.com/pmelsted/bifrost)
//!   - [Fulgor](https://github.com/jermp/fulgor)
//!   - [Metagraph](https://github.com/ratschlab/metagraph)
//!   - [SAM](https://samtools.github.io/hts-specs/SAMv1.pdf)
//!   - [Themisto](https://github.com/algbio/themisto)
//!
//! For details on each input format, see [Format]. We welcome contributions
//! implementing support for new tools but recommend first investigating whether
//! one of the existing formats fits your needs.
//!

use headers::file::FileHeader;
use headers::file::FileFlags;
use headers::block::BlockFlags;
use headers::block::read_block_header;
use headers::file::read_file_header;
use headers::file::read_file_flags;
use headers::file::encode_file_flags;
use headers::file::encode_file_header;

use std::io::Read;
use std::io::Write;

use roaring::bitmap::RoaringBitmap;

pub mod headers;
pub mod decoder;
pub mod encoder;
pub mod pack;
pub mod parser;
pub mod printer;
pub mod unpack;

type E = Box<dyn std::error::Error>;

/// Supported formats
///
/// Encoded as a 16-bit integer in [FileHeader] with the following mapping:
///
///   - 0: Unknown
///   - 1: [Themisto](https://github.com/algbio/themisto)
///
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

#[non_exhaustive]
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PseudoAln{
    pub ones: Option<Vec<u32>>,
    pub ones_names: Option<Vec<String>>,
    pub query_id: Option<u32>,
    pub query_name: Option<String>,
}

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

    // TODO Think if this makes sense or if it would be better to rename the query
    let query_name = headers_flags[0].1.query_name.clone();

    headers_flags.iter().for_each(|(header, flags)| {
        n_queries += header.n_queries;
        assert_eq!(n_targets, header.n_targets);
        assert_eq!(target_names, flags.target_names);
    });

    let new_flags = FileFlags { query_name, target_names };
    let new_flags_bytes = encode_file_flags(&new_flags)?;
    let new_header_bytes = encode_file_header(n_targets, n_queries, new_flags_bytes.len() as u32, 0_u16, 0_u16, 0_u64, 0_u64)?;

    conn_out.write_all(&new_header_bytes)?;
    conn_out.write_all(&new_flags_bytes)?;

    // TODO Need to update query ids in BlockFlags
    //
    // Do we want to consider duplicated queries with the same ID as the original?
    // yes?

    conns.iter_mut().for_each(|conn_in| {
        std::io::copy(conn_in, conn_out).unwrap();
    });

    conn_out.flush()?;

    Ok(())
}

/// Convert from [Read] to [Write]
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
    let mut writer = crate::printer::Printer::new(&mut reader, header, flags, format);
    for record in writer.by_ref() {
        conn_out.write_all(&record)?;
    }
    Ok(())
}

/// Encode from memory to something that implements [Write](std::io::Write).
pub fn encode_to_write<W: Write>(
    file_header: &FileHeader,
    records: &[PseudoAln],
    conn: &mut W,
) -> Result<(), E> {
    assert!(!records.is_empty());

    let packed = pack::pack(file_header, records)?;
    conn.write_all(&packed)?;
    conn.flush()?;

    Ok(())
}

/// Parse all plain-text pseudoalignments from [Read](std::io::Read) and encode to memory.
pub fn encode_from_read<R: Read>(
    targets: &[String],
    queries: &[String],
    sample_name: &str,
    conn_in: &mut R,
) -> Result<Vec<u8>, E> {
    let mut reader = crate::parser::Parser::new(conn_in, targets, queries, sample_name)?;
    let mut encoder = encoder::Encoder::new(&mut reader, targets, queries, sample_name);

    let mut bytes = encoder.encode_header_and_flags().unwrap();
    while let Some(mut block) = encoder.next_block() {
        bytes.append(&mut block);
    }
    Ok(bytes)
}

/// Parse all plain-text pseudoalignments from [Read](std::io::Read) and encode to [Write](std::io::Write).
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
    while let Some(bytes) = encoder.next_block() {
        conn_out.write_all(&bytes)?;
    }
    conn_out.flush()?;
    Ok(())
}

/// Decode all pseudoalignments from [Read](std::io::Read) and format to [Write](std::io::Write).
pub fn decode_from_read_to_write<R: Read, W: Write>(
    out_format: Format,
    conn_in: &mut R,
    conn_out: &mut W,
) -> Result<(), E> {
    let decoder = decoder::Decoder::new(conn_in);

    let header = decoder.file_header().clone();
    let flags = decoder.file_flags().clone();
    for block in decoder {
        let mut block_iter = block.into_iter();
        let printer = printer::Printer::new(&mut block_iter, header.clone(), flags.clone(), out_format.clone());
        for line in printer {
            conn_out.write_all(&line)?;
        }
    }

    conn_out.flush()?;
    Ok(())
}

/// Decode all pseudoalignments from [Read](std::io::Read) to memory.
pub fn decode_from_read<R: Read>(
    conn_in: &mut R,
) -> Result<(FileHeader, FileFlags, Vec<PseudoAln>), E> {

    let file_header = read_file_header(conn_in).unwrap();
    let file_flags = read_file_flags(&file_header, conn_in).unwrap();

    let mut res: Vec<PseudoAln> = Vec::with_capacity(file_header.n_queries as usize);
    while let Ok(block_header) = read_block_header(conn_in) {
        let mut bytes: Vec<u8> = vec![0; block_header.deflated_len as usize];
        conn_in.read_exact(&mut bytes)?;
        let (bitmap, block_flags) = unpack::unpack(&bytes, &block_header)?;
        let mut alns = unpack::decode_from_roaring(&bitmap, &file_flags, &block_header, &block_flags, file_header.n_targets)?;
        res.append(&mut alns);
    }

    Ok((file_header, file_flags, res))
}

/// Decode from memory and format to [Write](std::io::Write).
pub fn decode_to_write<R: Read, W: Write>(
    out_format: Format,
    records: &[u8],
    conn_out: &mut W,
) -> Result<(), E> {
    let mut tmp = std::io::Cursor::new(&records);
    let decoder = decoder::Decoder::new(&mut tmp);

    let header = decoder.file_header().clone();
    let flags = decoder.file_flags().clone();
    for block in decoder {
        let mut block_iter = block.into_iter();
        let printer = printer::Printer::new(&mut block_iter, header.clone(), flags.clone(), out_format.clone());
        for line in printer {
            conn_out.write_all(&line)?;
        }
    }

    conn_out.flush()?;
    Ok(())
}

/// Reads the full bitmap and combined block flags from a file
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

        let (bitmap, mut block_flags) = unpack::unpack(&block_bytes, &block_header)?;

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
/// Doesn't check that the bitmaps are compatible.
///
pub fn decode_from_read_into_roaring<R: Read>(
    conn_in: &mut R,
    bitmap_out: &mut RoaringBitmap,
) -> Result<(), E> {
    let header = crate::headers::file::read_file_header(conn_in)?;
    crate::headers::file::read_file_flags(&header, conn_in)?;

    while let Ok(block_header) = read_block_header(conn_in) {
        let mut block_bytes: Vec<u8> = vec![0; block_header.deflated_len as usize];
        conn_in.read_exact(&mut block_bytes)?;

        let (bitmap, _block_flags) = unpack::unpack(&block_bytes, &block_header)?;

        *bitmap_out |= bitmap;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn encode_to_write() {
        use super::encode_to_write;

        use super::headers::file::build_header_and_flags;
        use super::headers::file::encode_header_and_flags;

        use crate::PseudoAln;

        use std::io::Cursor;
        use std::io::Write;

        let data = vec![
            PseudoAln{ones_names: Some(vec!["chr.fasta".to_string()]),  query_id: Some(1), ones: Some(vec![0]), query_name: Some("ERR4035126.2".to_string()) },
            PseudoAln{ones_names: Some(vec!["chr.fasta".to_string()]),  query_id: Some(0), ones: Some(vec![0]), query_name: Some("ERR4035126.1".to_string()) },
            PseudoAln{ones_names: Some(vec!["chr.fasta".to_string(), "plasmid.fasta".to_string()]),  query_id: Some(2), ones: Some(vec![0, 1]), query_name: Some("ERR4035126.651903".to_string()) },
            PseudoAln{ones_names: Some(vec![]),  query_id: Some(4), ones: Some(vec![]), query_name: Some("ERR4035126.16".to_string()) },
            PseudoAln{ones_names: Some(vec!["plasmid.fasta".to_string()]),  query_id: Some(3), ones: Some(vec![1]), query_name: Some("ERR4035126.7543".to_string()) },
        ];
        let mut bytes: Cursor<Vec<u8>> = Cursor::new(Vec::new());

        let (header, flags) = build_header_and_flags(&vec!["chr.fasta".to_string(), "plasmid.fasta".to_string()], &vec!["ERR4035126.1".to_string(), "ERR4035126.2".to_string(), "ERR4035126.651903".to_string(), "ERR4035126.7543".to_string(), "ERR4035126.16".to_string()], &"ERR4035126".to_string()).unwrap();

        let header_flags_bytes = encode_header_and_flags(&header, &flags).unwrap();
        bytes.write(&header_flags_bytes).unwrap();
        encode_to_write(&header, &data, &mut bytes).unwrap();

        let expected: Vec<u8> = vec![2, 0, 0, 0, 5, 0, 0, 0, 36, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 10, 69, 82, 82, 52, 48, 51, 53, 49, 50, 54, 2, 9, 99, 104, 114, 46, 102, 97, 115, 116, 97, 13, 112, 108, 97, 115, 109, 105, 100, 46, 102, 97, 115, 116, 97, 5, 0, 0, 0, 102, 0, 0, 0, 26, 0, 0, 0, 81, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 147, 239, 230, 96, 0, 131, 255, 155, 141, 18, 18, 18, 82, 24, 24, 197, 216, 24, 13, 206, 30, 57, 112, 232, 192, 169, 3, 39, 15, 156, 122, 44, 37, 146, 146, 148, 144, 147, 149, 145, 178, 44, 189, 229, 140, 161, 136, 203, 163, 25, 51, 165, 162, 164, 36, 62, 43, 121, 207, 254, 168, 252, 241, 140, 175, 111, 79, 164, 164, 228, 140, 136, 25, 140, 102, 251, 13, 119, 102, 51, 48, 48, 0, 0, 158, 168, 250, 0, 82, 0, 0, 0];

        assert_eq!(*bytes.get_ref(), expected);
    }

    #[test]
    fn encode_from_read() {
        use super::encode_from_read;

        use super::headers::file::build_header_and_flags;
        use super::headers::file::encode_header_and_flags;

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

        let expected: Vec<u8> = vec![2, 0, 0, 0, 5, 0, 0, 0, 36, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 10, 69, 82, 82, 52, 48, 51, 53, 49, 50, 54, 2, 9, 99, 104, 114, 46, 102, 97, 115, 116, 97, 13, 112, 108, 97, 115, 109, 105, 100, 46, 102, 97, 115, 116, 97, 5, 0, 0, 0, 103, 0, 0, 0, 26, 0, 0, 0, 81, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 147, 239, 230, 96, 0, 131, 255, 155, 141, 18, 18, 18, 82, 24, 24, 197, 216, 24, 13, 206, 30, 57, 112, 232, 192, 169, 3, 231, 14, 156, 122, 44, 37, 146, 146, 148, 144, 147, 149, 145, 178, 44, 189, 227, 140, 161, 144, 203, 163, 25, 51, 165, 162, 164, 36, 62, 43, 119, 206, 152, 61, 75, 226, 179, 210, 107, 211, 228, 212, 132, 148, 164, 52, 70, 134, 146, 247, 91, 214, 102, 51, 48, 48, 0, 0, 206, 10, 209, 169, 83, 0, 0, 0];

        let targets = vec!["chr.fasta".to_string(), "plasmid.fasta".to_string()];
        let queries = vec!["ERR4035126.1".to_string(), "ERR4035126.2".to_string(), "ERR4035126.651903".to_string(), "ERR4035126.7543".to_string(), "ERR4035126.16".to_string()];
        let query_name ="ERR4035126".to_string();

        let (header, flags) = build_header_and_flags(&targets, &queries, &query_name).unwrap();
        let format = Format::Metagraph;

        let mut tmp = data.into_iter();
        let mut writer = crate::printer::Printer::new(&mut tmp, header, flags, format);
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

        use super::headers::file::build_header_and_flags;
        use super::headers::file::encode_header_and_flags;

        use crate::PseudoAln;
        use crate::Format;

        use std::io::Cursor;
        use std::io::Seek;
        use std::io::Write;

        let data_bytes: Vec<u8> = vec![49, 9, 69, 82, 82, 52, 48, 51, 53, 49, 50, 54, 46, 50, 9, 99, 104, 114, 46, 102, 97, 115, 116, 97, 10, 48, 9, 69, 82, 82, 52, 48, 51, 53, 49, 50, 54, 46, 49, 9, 99, 104, 114, 46, 102, 97, 115, 116, 97, 10, 50, 9, 69, 82, 82, 52, 48, 51, 53, 49, 50, 54, 46, 54, 53, 49, 57, 48, 51, 9, 99, 104, 114, 46, 102, 97, 115, 116, 97, 58, 112, 108, 97, 115, 109, 105, 100, 46, 102, 97, 115, 116, 97, 10, 52, 9, 69, 82, 82, 52, 48, 51, 53, 49, 50, 54, 46, 49, 54, 9, 10, 51, 9, 69, 82, 82, 52, 48, 51, 53, 49, 50, 54, 46, 55, 53, 52, 51, 9, 112, 108, 97, 115, 109, 105, 100, 46, 102, 97, 115, 116, 97, 10];
        let mut data = Cursor::new(data_bytes);

        let expected: Vec<u8> = vec![2, 0, 0, 0, 5, 0, 0, 0, 36, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 10, 69, 82, 82, 52, 48, 51, 53, 49, 50, 54, 2, 9, 99, 104, 114, 46, 102, 97, 115, 116, 97, 13, 112, 108, 97, 115, 109, 105, 100, 46, 102, 97, 115, 116, 97, 5, 0, 0, 0, 103, 0, 0, 0, 26, 0, 0, 0, 81, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 147, 239, 230, 96, 0, 131, 255, 155, 141, 18, 18, 18, 82, 24, 24, 197, 216, 24, 13, 206, 30, 57, 112, 232, 192, 169, 3, 231, 14, 156, 122, 44, 37, 146, 146, 148, 144, 147, 149, 145, 178, 44, 189, 227, 140, 161, 144, 203, 163, 25, 51, 165, 162, 164, 36, 62, 43, 119, 206, 152, 61, 75, 226, 179, 210, 107, 211, 228, 212, 132, 148, 164, 52, 70, 134, 146, 247, 91, 214, 102, 51, 48, 48, 0, 0, 206, 10, 209, 169, 83, 0, 0, 0];

        let targets = vec!["chr.fasta".to_string(), "plasmid.fasta".to_string()];
        let queries = vec!["ERR4035126.1".to_string(), "ERR4035126.2".to_string(), "ERR4035126.651903".to_string(), "ERR4035126.7543".to_string(), "ERR4035126.16".to_string()];
        let query_name ="ERR4035126".to_string();

        let (header, flags) = build_header_and_flags(&targets, &queries, &query_name).unwrap();
        let format = Format::Metagraph;

        let mut tmp = data.into_iter();
        let mut writer = crate::printer::Printer::new(&mut tmp, header, flags, format);
        for record in writer.by_ref() {
            bytes.write(&record).unwrap();
        }
        bytes.rewind().unwrap();

        let mut bytes_got: Cursor<Vec<u8>> = Cursor::new(Vec::new());
        encode_from_read_to_write(&targets, &queries, &query_name, &mut bytes, &mut bytes_got).unwrap();
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

        let data: Vec<u8> = vec![2, 0, 0, 0, 5, 0, 0, 0, 36, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 10, 69, 82, 82, 52, 48, 51, 53, 49, 50, 54, 2, 9, 99, 104, 114, 46, 102, 97, 115, 116, 97, 13, 112, 108, 97, 115, 109, 105, 100, 46, 102, 97, 115, 116, 97, 5, 0, 0, 0, 102, 0, 0, 0, 26, 0, 0, 0, 81, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 147, 239, 230, 96, 0, 131, 255, 155, 141, 18, 18, 18, 82, 24, 24, 197, 216, 24, 13, 206, 30, 57, 112, 232, 192, 169, 3, 39, 15, 156, 122, 44, 37, 146, 146, 148, 144, 147, 149, 145, 178, 44, 189, 229, 140, 161, 136, 203, 163, 25, 51, 165, 162, 164, 36, 62, 43, 121, 207, 254, 168, 252, 241, 140, 175, 111, 79, 164, 164, 228, 140, 136, 25, 140, 102, 251, 13, 119, 102, 51, 48, 48, 0, 0, 158, 168, 250, 0, 82, 0, 0, 0];
        let mut bytes: Cursor<Vec<u8>> = Cursor::new(data);

        let (file_header, file_flags, mut got) = decode_from_read(&mut bytes).unwrap();
        got.sort_by_key(|x| *x.query_id.as_ref().unwrap());

        assert_eq!(expected_header, file_header);
        assert_eq!(expected_flags, file_flags);
        assert_eq!(expected_alns, got);
    }
