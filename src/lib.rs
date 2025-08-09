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

use std::io::BufRead;
use std::io::BufReader;
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
pub enum Format {
    // Bifrost,
    // Fulgor,
    // Metagraph,
    // SAM,
    Themisto,
}

#[non_exhaustive]
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PseudoAln {
    pub ones: Vec<bool>,
    pub query_id: Option<u32>,
    pub query_name: Option<String>,
}

pub fn parse<R: Read>(
    num_targets: usize,
    conn: &mut R,
) -> Vec<PseudoAln> {
    let reader = BufReader::new(conn);

    let res: Vec<PseudoAln> = reader.lines().map(|line| {
            parser::themisto::read_themisto(num_targets, &mut line.unwrap().as_bytes()).unwrap()
    }).collect();

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
    let n_targets = records[0].ones.len();

    let flags_bytes = encode_file_flags(targets, query_name)?;
    let file_header = encode_file_header(n_targets as u32, n_queries as u32, flags_bytes.len() as u32, 1, 0,0,0)?;

    conn.write_all(&file_header)?;
    conn.write_all(&flags_bytes)?;

    let packed = pack::pack(records)?;
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
            PseudoAln{ query_id: Some(128), ones: vec![true, false, false, true, false, false, false, true, false, false, false, true], ..Default::default()},
            PseudoAln{ query_id: Some(7),   ones: vec![true, true, true, true, false, false, false, false, false, false, false, false], ..Default::default()},
            PseudoAln{ query_id: Some(8),   ones: vec![false, false, false, false, false, false, false, false, false, false, false, false], ..Default::default()},
            PseudoAln{ query_id: Some(0),   ones: vec![false, false, false, false, false, false, false, false, false, false, false, false], ..Default::default()},
            PseudoAln{ query_id: Some(1),   ones: vec![false, false, true, false, true, false, false, true, false, true, false, false], ..Default::default()},
        ];

        let mut input: Cursor<Vec<u8>> = Cursor::new(data);
        let got = parse(12, &mut input);

        assert_eq!(got, expected);
    }
}
