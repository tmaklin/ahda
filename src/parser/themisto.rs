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
use std::io::Read;

use crate::PseudoAln;

type E = Box<dyn std::error::Error>;

/// Parse a line from Themisto
///
/// Reads a pseudoalignment line stored in the *Themisto* format.
///
/// Returns the number of pseudoalignments on the line.
///
pub fn read_themisto<R: Read>(
    num_targets: usize,
    conn: &mut R,
) -> Result<PseudoAln, E> {
    let separator: char = ' ';
    let mut contents: String = String::new();
    conn.read_to_string(&mut contents)?;

    let mut records = contents.split(separator);

    let id_bytes = records.next().unwrap(); // TODO handle empty input
    let read_id = id_bytes.parse::<u32>()?;

    let mut ones: Vec<bool> = vec![false; num_targets];

    for record in records {
        let id = record.parse::<u32>()?;
        ones[id as usize] = true;
    }

    let res = PseudoAln{ read_id, ones, ..Default::default()};
    Ok(res)
}

// Tests
#[cfg(test)]
mod tests {

    #[test]
    fn read_themisto_line_multiple_aligned() {
        use std::io::Cursor;
        use crate::PseudoAln;
        use super::read_themisto;

        let data: Vec<u8> = b"128 0 7 11 3".to_vec();
        let expected = PseudoAln{ read_id: 128, ones: vec![true, false, false, true, false, false, false, true, false, false, false, true], ..Default::default()};

        let mut input: Cursor<Vec<u8>> = Cursor::new(data);
        let got = read_themisto(12, &mut input).unwrap();

        assert_eq!(got, expected);
    }

    #[test]
    fn read_themisto_line_empty() {
        use std::io::Cursor;
        use crate::PseudoAln;
        use super::read_themisto;

        let data: Vec<u8> = b"185216".to_vec();
        let expected = PseudoAln{ read_id: 185216, ones: vec![false; 2], ..Default::default()};

        let mut input: Cursor<Vec<u8>> = Cursor::new(data);
        let got = read_themisto(2, &mut input).unwrap();

        assert_eq!(got, expected);
    }

    #[test]
    fn read_themisto_multiple() {
        use crate::PseudoAln;
        use super::read_themisto;
        use std::io::BufRead;
        use std::io::BufReader;
        use std::io::Cursor;

        let data: Vec<u8> = b"185216\n188352\n202678 1\n202728\n651964 0 1\n651966 0 1\n1166624 0\n1166625 0\n1166626 1".to_vec();
        let expected = vec![
            PseudoAln{ read_id: 185216, ones: vec![false; 2], ..Default::default()},
            PseudoAln{ read_id: 188352, ones: vec![false; 2], ..Default::default()},
            PseudoAln{ read_id: 202678, ones: vec![false, true], ..Default::default()},
            PseudoAln{ read_id: 202728, ones: vec![false, false], ..Default::default()},
            PseudoAln{ read_id: 651964, ones: vec![true, true], ..Default::default()},
            PseudoAln{ read_id: 651966, ones: vec![true, true], ..Default::default()},
            PseudoAln{ read_id: 1166624, ones: vec![true, false], ..Default::default()},
            PseudoAln{ read_id: 1166625, ones: vec![true, false], ..Default::default()},
            PseudoAln{ read_id: 1166626, ones: vec![false, true], ..Default::default()},
        ];

        let cursor = Cursor::new(data);
        let reader = BufReader::new(cursor);
        let got: Vec<PseudoAln> = reader.lines().map(|line| {
            read_themisto(2, &mut line.unwrap().as_bytes()).unwrap()
        }).collect();

        assert_eq!(got, expected);
    }
}
