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
use headers::block::read_block_header;
use headers::file::encode_file_flags;
use headers::file::encode_file_header;
use headers::file::read_file_header;

use parser::Parser;

use std::io::Read;
use std::io::Write;

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
    // Metagraph,
    // SAM,
    Themisto,
}

#[non_exhaustive]
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PseudoAln {
    pub ones: Vec<u32>,
    pub query_id: Option<u32>,
    pub query_name: Option<String>,
}

pub fn parse<R: Read>(
    conn: &mut R,
) -> Vec<PseudoAln> {
    let mut reader = Parser::new(conn);

    let mut res: Vec<PseudoAln> = Vec::new();
    while let Some(record) = reader.next() {
        res.push(record);
    }

    res
}

/// Write pseudoalignments in .ahda format
pub fn encode<W: Write>(
    records: &[PseudoAln],
    targets: &[String],
    query_name: &str,
    n_queries: usize,
    conn: &mut W,
) -> Result<(), E> {
    assert!(!records.is_empty());
    let n_targets = targets.len();

    let flags_bytes = encode_file_flags(targets, query_name)?;
    let file_header = encode_file_header(n_targets as u32, n_queries as u32, flags_bytes.len() as u32, 1, 0,0,0)?;

    conn.write_all(&file_header)?;
    conn.write_all(&flags_bytes)?;

    let packed = pack::pack(records, n_targets)?;
    conn.write_all(&packed)?;
    conn.flush()?;

    Ok(())
}

pub fn decode<R: Read>(
    conn: &mut R,
) -> Result<Vec<PseudoAln>, E> {
    let file_header = read_file_header(conn).unwrap();

    let mut dump: Vec<u8> = vec![0; file_header.flags_len as usize];
    let _ = conn.read_exact(&mut dump);

    let block_header = read_block_header(conn)?;
    dump = vec![0; block_header.flags_len as usize];
    let _ = conn.read_exact(&mut dump);

    let res: Vec<PseudoAln> = unpack::unpack(&block_header, file_header.n_targets as usize, conn)?;

    Ok(res)
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
            PseudoAln{ query_id: Some(128), ones: vec![0, 7, 11, 3], ..Default::default()},
            PseudoAln{ query_id: Some(7),   ones: vec![3, 2, 1, 0], ..Default::default()},
            PseudoAln{ query_id: Some(8),   ones: vec![], ..Default::default()},
            PseudoAln{ query_id: Some(0),   ones: vec![], ..Default::default()},
            PseudoAln{ query_id: Some(1),   ones: vec![4, 2, 9, 7], ..Default::default()},
        ];

        let mut input: Cursor<Vec<u8>> = Cursor::new(data);
        let got = parse(&mut input);

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
            PseudoAln{ query_id: None, ones: vec![], query_name: Some("ERR4035126.4996".to_string()) },
            PseudoAln{ query_id: None, ones: vec![0], query_name: Some("ERR4035126.1262953".to_string()) },
            PseudoAln{ query_id: None, ones: vec![1], query_name: Some("ERR4035126.1262954".to_string()) },
            PseudoAln{ query_id: None, ones: vec![1], query_name: Some("ERR4035126.1262955".to_string()) },
            PseudoAln{ query_id: None, ones: vec![0], query_name: Some("ERR4035126.1262956".to_string()) },
            PseudoAln{ query_id: None, ones: vec![0], query_name: Some("ERR4035126.1262957".to_string()) },
            PseudoAln{ query_id: None, ones: vec![0], query_name: Some("ERR4035126.1262958".to_string()) },
            PseudoAln{ query_id: None, ones: vec![0], query_name: Some("ERR4035126.1262959".to_string()) },
            PseudoAln{ query_id: None, ones: vec![0, 1], query_name: Some("ERR4035126.651965".to_string()) },
            PseudoAln{ query_id: None, ones: vec![], query_name: Some("ERR4035126.11302".to_string()) },
            PseudoAln{ query_id: None, ones: vec![0], query_name: Some("ERR4035126.1262960".to_string()) },
            PseudoAln{ query_id: None, ones: vec![0], query_name: Some("ERR4035126.1262961".to_string()) },
            PseudoAln{ query_id: None, ones: vec![0], query_name: Some("ERR4035126.1262962".to_string()) },
            PseudoAln{ query_id: None, ones: vec![0, 1], query_name: Some("ERR4035126.651965".to_string()) },
        ];

        let mut input: Cursor<Vec<u8>> = Cursor::new(data);
        let got = parse(&mut input);

        assert_eq!(got, expected);
    }
}
