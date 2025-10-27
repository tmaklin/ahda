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

#[non_exhaustive]
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PseudoAln{
    pub ones: Option<Vec<u32>>,
    pub ones_names: Option<Vec<String>>,
    pub query_id: Option<u32>,
    pub query_name: Option<String>,
}

pub fn parse<R: Read>(
    conn: &mut R,
) -> Result<(Vec<PseudoAln>, Format), E> {
    let mut reader = Parser::new(conn)?;

    let mut res: Vec<PseudoAln> = Vec::new();
    while let Some(record) = reader.next() {
        res.push(record);
    }

    Ok((res, reader.format))
}

/// Write pseudoalignments in .ahda format to a writer that implements `std::io::Write`
pub fn encode_block<W: Write>(
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

/// Decodes a single .ahda block from a reader that implements `std::io::Read`
pub fn decode_block_from_std_read<R: Read>(
    file_flags: &FileFlags,
    conn: &mut R,
) -> Result<Vec<PseudoAln>, E> {
    let block_header = read_block_header(conn)?;
    unpack::unpack(&block_header, &file_flags, conn)
}

/// Decodes a complete .ahda file from a reader that implements `std::io::Read`
pub fn decode_file_from_std_read<R: Read>(
    file_flags: &FileFlags,
    conn: &mut R,
) -> Result<Vec<PseudoAln>, E> {

    let file_header = read_file_header(conn).unwrap();

    let mut dump: Vec<u8> = vec![0; file_header.flags_len as usize];
    let _ = conn.read_exact(&mut dump);

    let mut res: Vec<PseudoAln> = Vec::with_capacity(file_header.n_queries as usize);
    while let Ok(block_header) = read_block_header(conn) {
        res.append(&mut unpack::unpack(&block_header, &file_flags, conn)?);
    }

    todo!("Implement decode_file_from_std_read"); // This function is broken
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

        let mut block_flags = decode_block_flags(&flags_bytes)?;
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

// Tests
#[cfg(test)]
mod tests {

    #[test]
    fn parse_themisto_output() {
        use crate::Format;
        use std::io::Cursor;
        use super::parse;
        use super::PseudoAln;

        let data: Vec<u8> = vec![b"128 0 7 11 3\n".to_vec(),
                                 b"7 3 2 1 0\n".to_vec(),
                                 b"8\n".to_vec(),
                                 b"0\n".to_vec(),
                                 b"1 4 2 9 7\n".to_vec(),
        ].concat();

        let expected = vec![
            PseudoAln{ones_names: None,  query_id: Some(128), ones: Some(vec![0, 7, 11, 3]), ..Default::default()},
            PseudoAln{ones_names: None,  query_id: Some(7),   ones: Some(vec![3, 2, 1, 0]), ..Default::default()},
            PseudoAln{ones_names: None,  query_id: Some(8),   ones: Some(vec![]), ..Default::default()},
            PseudoAln{ones_names: None,  query_id: Some(0),   ones: Some(vec![]), ..Default::default()},
            PseudoAln{ones_names: None,  query_id: Some(1),   ones: Some(vec![4, 2, 9, 7]), ..Default::default()},
        ];

        let mut input: Cursor<Vec<u8>> = Cursor::new(data);
        let (got, got_format) = parse(&mut input).unwrap();

        assert_eq!(got_format, Format::Themisto);
        assert_eq!(got, expected);
    }

    #[test]
    fn parse_fulgor_output() {
        use crate::Format;
        use std::io::Cursor;

        use super::parse;
        use super::PseudoAln;

        let mut data: Vec<u8> = b"ERR4035126.4996\t0\n".to_vec();
        data.append(&mut b"ERR4035126.1262953\t1\t0\n".to_vec());
        data.append(&mut b"ERR4035126.1262954\t1\t1\n".to_vec());
        data.append(&mut b"ERR4035126.1262955\t1\t1\n".to_vec());
        data.append(&mut b"ERR4035126.1262956\t1\t0\n".to_vec());
        data.append(&mut b"ERR4035126.1262957\t1\t0\n".to_vec());
        data.append(&mut b"ERR4035126.1262958\t1\t0\n".to_vec());
        data.append(&mut b"ERR4035126.1262959\t1\t0\n".to_vec());
        data.append(&mut b"ERR4035126.651965\t2\t0\t1\n".to_vec());
        data.append(&mut b"ERR4035126.11302\t0\n".to_vec());
        data.append(&mut b"ERR4035126.1262960\t1\t0\n".to_vec());
        data.append(&mut b"ERR4035126.1262961\t1\t0\n".to_vec());
        data.append(&mut b"ERR4035126.1262962\t1\t0\n".to_vec());
        data.append(&mut b"ERR4035126.651965\t2\t0\t1\n".to_vec());

        let expected = vec![
            PseudoAln{ones_names: None,  query_id: None, ones: Some(vec![]), query_name: Some("ERR4035126.4996".to_string()) },
            PseudoAln{ones_names: None,  query_id: None, ones: Some(vec![0]), query_name: Some("ERR4035126.1262953".to_string()) },
            PseudoAln{ones_names: None,  query_id: None, ones: Some(vec![1]), query_name: Some("ERR4035126.1262954".to_string()) },
            PseudoAln{ones_names: None,  query_id: None, ones: Some(vec![1]), query_name: Some("ERR4035126.1262955".to_string()) },
            PseudoAln{ones_names: None,  query_id: None, ones: Some(vec![0]), query_name: Some("ERR4035126.1262956".to_string()) },
            PseudoAln{ones_names: None,  query_id: None, ones: Some(vec![0]), query_name: Some("ERR4035126.1262957".to_string()) },
            PseudoAln{ones_names: None,  query_id: None, ones: Some(vec![0]), query_name: Some("ERR4035126.1262958".to_string()) },
            PseudoAln{ones_names: None,  query_id: None, ones: Some(vec![0]), query_name: Some("ERR4035126.1262959".to_string()) },
            PseudoAln{ones_names: None,  query_id: None, ones: Some(vec![0, 1]), query_name: Some("ERR4035126.651965".to_string()) },
            PseudoAln{ones_names: None,  query_id: None, ones: Some(vec![]), query_name: Some("ERR4035126.11302".to_string()) },
            PseudoAln{ones_names: None,  query_id: None, ones: Some(vec![0]), query_name: Some("ERR4035126.1262960".to_string()) },
            PseudoAln{ones_names: None,  query_id: None, ones: Some(vec![0]), query_name: Some("ERR4035126.1262961".to_string()) },
            PseudoAln{ones_names: None,  query_id: None, ones: Some(vec![0]), query_name: Some("ERR4035126.1262962".to_string()) },
            PseudoAln{ones_names: None,  query_id: None, ones: Some(vec![0, 1]), query_name: Some("ERR4035126.651965".to_string()) },
        ];

        let mut input: Cursor<Vec<u8>> = Cursor::new(data);
        let (got, got_format) = parse(&mut input).unwrap();

        assert_eq!(got_format, Format::Fulgor);
        assert_eq!(got, expected);
    }

    #[test]
    fn parse_bifrost_output() {
        use crate::Format;
        use std::io::Cursor;

        use super::parse;
        use super::PseudoAln;

        let mut data: Vec<u8> = b"query_name\tchromosome.fasta\tplasmid.fasta\n".to_vec();
        data.append(&mut b"ERR4035126.724962\t0\t0\n".to_vec());
        data.append(&mut b"ERR4035126.1235744\t0\t0\n".to_vec());
        data.append(&mut b"ERR4035126.431001\t0\t0\n".to_vec());
        data.append(&mut b"ERR4035126.645400\t0\t0\n".to_vec());
        data.append(&mut b"ERR4035126.3001\t121\t0\n".to_vec());
        data.append(&mut b"ERR4035126.515778\t242\t0\n".to_vec());
        data.append(&mut b"ERR4035126.886205\t121\t0\n".to_vec());
        data.append(&mut b"ERR4035126.1254676\t121\t0\n".to_vec());
        data.append(&mut b"ERR4035126.668031\t0\t121\n".to_vec());
        data.append(&mut b"ERR4035126.388619\t121\t0\n".to_vec());
        data.append(&mut b"ERR4035126.959743\t0\t0\n".to_vec());
        data.append(&mut b"ERR4035126.1146685\t0\t0\n".to_vec());
        data.append(&mut b"ERR4035126.1017809\t0\t0\n".to_vec());
        data.append(&mut b"ERR4035126.788136\t0\t0\n".to_vec());
        data.append(&mut b"ERR4035126.1223924\t366\t9\n".to_vec());
        data.append(&mut b"ERR4035126.910807\t0\t0\n".to_vec());
        data.append(&mut b"ERR4035126.824748\t80\t0\n".to_vec());

        let expected = vec![
            PseudoAln{ query_name: Some("ERR4035126.724962".to_string()), ones: Some(vec![]), ones_names: None, query_id: None },
            PseudoAln{ query_name: Some("ERR4035126.1235744".to_string()), ones: Some(vec![]), ones_names: None, query_id: None },
            PseudoAln{ query_name: Some("ERR4035126.431001".to_string()), ones: Some(vec![]), ones_names: None, query_id: None },
            PseudoAln{ query_name: Some("ERR4035126.645400".to_string()), ones: Some(vec![]), ones_names: None, query_id: None },
            PseudoAln{ query_name: Some("ERR4035126.3001".to_string()), ones: Some(vec![0]), ones_names: None, query_id: None },
            PseudoAln{ query_name: Some("ERR4035126.515778".to_string()), ones: Some(vec![0]), ones_names: None, query_id: None },
            PseudoAln{ query_name: Some("ERR4035126.886205".to_string()), ones: Some(vec![0]), ones_names: None, query_id: None },
            PseudoAln{ query_name: Some("ERR4035126.1254676".to_string()), ones: Some(vec![0]), ones_names: None, query_id: None },
            PseudoAln{ query_name: Some("ERR4035126.668031".to_string()), ones: Some(vec![1]), ones_names: None, query_id: None },
            PseudoAln{ query_name: Some("ERR4035126.388619".to_string()), ones: Some(vec![0]), ones_names: None, query_id: None },
            PseudoAln{ query_name: Some("ERR4035126.959743".to_string()), ones: Some(vec![]), ones_names: None, query_id: None },
            PseudoAln{ query_name: Some("ERR4035126.1146685".to_string()), ones: Some(vec![]), ones_names: None, query_id: None },
            PseudoAln{ query_name: Some("ERR4035126.1017809".to_string()), ones: Some(vec![]), ones_names: None, query_id: None },
            PseudoAln{ query_name: Some("ERR4035126.788136".to_string()), ones: Some(vec![]), ones_names: None, query_id: None },
            PseudoAln{ query_name: Some("ERR4035126.1223924".to_string()), ones: Some(vec![0, 1]), ones_names: None, query_id: None },
            PseudoAln{ query_name: Some("ERR4035126.910807".to_string()), ones: Some(vec![]), ones_names: None, query_id: None },
            PseudoAln{ query_name: Some("ERR4035126.824748".to_string()), ones: Some(vec![0]), ones_names: None, query_id: None },
        ];

        let mut input: Cursor<Vec<u8>> = Cursor::new(data);
        let (got, got_format) = parse(&mut input).unwrap();

        assert_eq!(got_format, Format::Bifrost);
        assert_eq!(got, expected);
    }

    #[test]
    fn parse_metgraph_output() {
        use crate::Format;
        use std::io::Cursor;

        use super::parse;
        use super::PseudoAln;

        assert_eq!(0, 1);
    }
}
