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
use format::BlockHeader;
use format::FileHeader;
use format::read_block_header;

use std::io::BufRead;
use std::io::BufReader;
use std::io::Read;
use std::io::Write;

pub mod format;
pub mod pack;
pub mod parser;
pub mod printer;
pub mod unpack;

type E = Box<dyn std::error::Error>;

/// Supported formats
#[non_exhaustive]
pub enum Format {
    // Bifrost,
    // Fulgor,
    // Metagraph,
    // SAM,
    Themisto,
}

#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PseudoAln {
    pub read_id: u32,
    pub ones: Vec<bool>,
}

pub fn parse<R: Read>(
    num_targets: usize,
    conn: &mut R,
) -> Vec<PseudoAln> {
    let reader = BufReader::new(conn);

    let res: Vec<PseudoAln> = reader.lines().map(|line| {
            parser::read_themisto(12, &mut line.unwrap().as_bytes()).unwrap()
    }).collect();

    res
}

pub fn encode<W: Write>(
    records: &[PseudoAln],
    conn: &mut W,
) -> Result<(), E> {
    let packed = pack::pack(records)?;
    conn.write_all(&packed)?;
    conn.flush()?;
    Ok(())
}

pub fn decode<R: Read>(
    _file_header: &FileHeader,
    conn: &mut R,
) -> Result<Vec<PseudoAln>, E> {
    let mut res: Vec<PseudoAln> = Vec::new();

    while let Ok(block_header) = read_block_header(conn) {
        let mut records = unpack::unpack(&block_header, conn)?;
        res.append(&mut records);
    }

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
            PseudoAln{ read_id: 128, ones: vec![true, false, false, true, false, false, false, true, false, false, false, true]},
            PseudoAln{ read_id: 7,   ones: vec![true, true, true, true, false, false, false, false, false, false, false, false]},
            PseudoAln{ read_id: 8,   ones: vec![false, false, false, false, false, false, false, false, false, false, false, false]},
            PseudoAln{ read_id: 0,   ones: vec![false, false, false, false, false, false, false, false, false, false, false, false]},
            PseudoAln{ read_id: 1,   ones: vec![false, false, true, false, true, false, false, true, false, true, false, false]},
        ];

        let mut input: Cursor<Vec<u8>> = Cursor::new(data);
        let got = parse(12, &mut input);

        assert_eq!(got, expected);
    }
}
