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
use headers::block::decode_block_flags;
use headers::file::read_file_header;

use parser::Parser;

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

/// Convert from [Read] to [Write]
pub fn convert_from_std_read_to_std_write<R: Read, W: Write>(
    targets: &[String],
    queries: &[String],
    sample_name: &str,
    format: Format,
    conn_in: &mut R,
    conn_out: &mut W,
) -> Result<(), E> {
    let mut reader = crate::parser::Parser::new(conn_in, targets, queries, sample_name)?;
    let mut records: Vec<PseudoAln> = Vec::new();
    for record in reader.by_ref() {
        records.push(record);
    }
    let mut tmp = records.iter();
    let mut writer = crate::printer::Printer::new(&mut tmp, reader.file_header().clone(), reader.file_flags().clone(), format);
    for record in writer.by_ref() {
        conn_out.write_all(&record)?;
    }
    Ok(())
}

/// Encode from memory to something that implements [Write](std::io::Write).
pub fn encode_to_std_write<W: Write>(
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
pub fn encode_from_std_read<R: Read, W: Write>(
    targets: &[String],
    queries: &[String],
    sample_name: &str,
    conn_in: &mut R,
) -> Result<Vec<u8>, E> {
    let mut reader = crate::parser::Parser::new(conn_in)?;
    let mut encoder = encoder::Encoder::new(&mut reader, targets, queries, sample_name);

    let mut bytes = encoder.encode_header_and_flags().unwrap();
    while let Some(mut block) = encoder.next_block() {
        bytes.append(&mut block);
    }
    Ok(bytes)
}

/// Parse all plain-text pseudoalignments from [Read](std::io::Read) and encode to [Write](std::io::Write).
pub fn encode_from_std_read_to_std_write<R: Read, W: Write>(
    targets: &[String],
    queries: &[String],
    sample_name: &str,
    conn_in: &mut R,
    conn_out: &mut W,
) -> Result<(), E> {
    let mut reader = crate::parser::Parser::new(conn_in)?;
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
pub fn decode_from_std_read_to_std_write<R: Read, W: Write>(
    out_format: Format,
    conn_in: &mut R,
    conn_out: &mut W,
) -> Result<(), E> {
    let decoder = decoder::Decoder::new(conn_in);

    let header = decoder.file_header().clone();
    let flags = decoder.file_flags().clone();
    for block in decoder {
        let mut block_iter = block.iter();
        let printer = printer::Printer::new(&mut block_iter, header.clone(), flags.clone(), out_format.clone());
        for line in printer {
            conn_out.write_all(&line)?;
        }
    }

    conn_out.flush()?;
    Ok(())
}

/// Decode all pseudoalignments from [Read](std::io::Read) to memory.
pub fn decode_from_std_read<R: Read>(
    file_flags: &FileFlags,
    conn_in: &mut R,
) -> Result<Vec<PseudoAln>, E> {

    let file_header = read_file_header(conn_in).unwrap();

    let mut dump: Vec<u8> = vec![0; file_header.flags_len as usize];
    let _ = conn_in.read_exact(&mut dump);

    let mut res: Vec<PseudoAln> = Vec::with_capacity(file_header.n_queries as usize);
    while let Ok(block_header) = read_block_header(conn_in) {
        let mut bytes: Vec<u8> = vec![0; block_header.deflated_len as usize];
        conn_in.read_exact(&mut bytes)?;
        res.append(&mut unpack::unpack(&bytes, &block_header, file_flags)?);
    }

    todo!("Implement decode_file_from_std_read"); // This function is broken
}

/// Decode from memory and format to [Write](std::io::Write).
pub fn decode_to_std_write<R: Read, W: Write>(
    out_format: Format,
    records: &[u8],
    conn_out: &mut W,
) -> Result<(), E> {
    let mut tmp = std::io::Cursor::new(&records);
    let decoder = decoder::Decoder::new(&mut tmp);

    let header = decoder.file_header().clone();
    let flags = decoder.file_flags().clone();
    for block in decoder {
        let mut block_iter = block.iter();
        let printer = printer::Printer::new(&mut block_iter, header.clone(), flags.clone(), out_format.clone());
        for line in printer {
            conn_out.write_all(&line)?;
        }
    }

    conn_out.flush()?;
    Ok(())
}

/// Reads the full bitmap and combined block flags from a file
pub fn read_bitmap<R: Read>(
    conn: &mut R,
) -> Result<(RoaringBitmap, BlockFlags), E> {
    let mut bitmap = RoaringBitmap::new();

    let mut queries: Vec<String> = Vec::new();
    let mut query_ids: Vec<u32> = Vec::new();

    while let Ok(block_header) = read_block_header(conn) {
        let mut deflated_bytes: Vec<u8> = vec![0; block_header.deflated_len as usize];
        conn.read_exact(&mut deflated_bytes)?;

        let inflated_bytes = unpack::inflate_bytes(&deflated_bytes)?;
        let inflated_bytes = unpack::inflate_bytes(&inflated_bytes)?;

        let flags_bytes = &inflated_bytes[(block_header.block_len as usize)..inflated_bytes.len()];
        let bitmap_bytes = &inflated_bytes[0..(block_header.block_len as usize)];

        let mut block_flags = decode_block_flags(flags_bytes)?;
        queries.append(&mut block_flags.queries);
        query_ids.append(&mut block_flags.query_ids);

        let bitmap_deser = RoaringBitmap::deserialize_from(bitmap_bytes);
        bitmap |= bitmap_deser?;
    }

    let mut both: Vec<(u32, String)> = queries.iter().zip(query_ids.iter()).map(|(name, idx)| (*idx, name.to_string())).collect();
    both.sort_by_key(|x| x.0);
    let queries: Vec<String> = both.iter().map(|x| x.1.to_string()).collect();
    let query_ids: Vec<u32> = both.iter().map(|x| x.0).collect();

    Ok((bitmap, BlockFlags{ queries, query_ids }))
}
