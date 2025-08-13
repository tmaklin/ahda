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
use std::io::Write;

use crate::PseudoAln;

type E = Box<dyn std::error::Error>;

#[derive(Debug, Clone)]
pub struct FulgorPrinterError;

impl std::fmt::Display for FulgorPrinterError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "invalid input to encode")
    }
}

impl std::error::Error for FulgorPrinterError {}

/// Format a single pseudoalignment in Fulgor format
///
/// Writes bytes containing the formatted line containing the contents of
/// `aln` to `conn`.
///
/// Terminates with a [FulgorPrinterError] if [PseudoAln::query_id] or [PseudoAln::ones] is None.
///
pub fn format_fulgor_line<W: Write>(
    aln: &PseudoAln,
    conn: &mut W,
) -> Result<(), E> {
    let separator: char = '\t';
    let mut formatted: String = String::new();

    if aln.ones.is_none() || aln.query_id.is_none() {
        return Err(Box::new(FulgorPrinterError{}))
    }

    formatted += &aln.query_name.clone().unwrap().to_string();
    formatted += &separator.to_string();
    formatted += &aln.ones.as_ref().unwrap().len().to_string();

    aln.ones.as_ref().unwrap().iter().for_each(|idx| {
        formatted += &separator.to_string();
        formatted += &idx.to_string();
    });
    formatted += "\n";

    conn.write_all(formatted.as_bytes())?;
    Ok(())
}

// Tests
#[cfg(test)]
mod tests {

    #[test]
    fn format_fulgor_line_1st_aligned() {
        use std::io::Cursor;
        use crate::PseudoAln;
        use super::format_fulgor_line;

        let data = PseudoAln{ones_names: None,  query_id: Some(1262953), ones: Some(vec![0]), query_name: Some("ERR4035126.1262954".to_string()) };

        let expected: Vec<u8> = b"ERR4035126.1262954\t1\t0\n".to_vec();

        let mut got: Vec<u8> = Vec::new();
        format_fulgor_line(&data, &mut got).unwrap();

        assert_eq!(got, expected);
    }

    #[test]
    fn format_fulgor_line_2nd_aligned() {
        use std::io::Cursor;
        use crate::PseudoAln;
        use super::format_fulgor_line;

        let data = PseudoAln{ones_names: None,  query_id: Some(1262953), ones: Some(vec![1]), query_name: Some("ERR4035126.1262954".to_string()) };

        let expected: Vec<u8> = b"ERR4035126.1262954\t1\t1\n".to_vec();

        let mut got: Vec<u8> = Vec::new();
        format_fulgor_line(&data, &mut got).unwrap();

        assert_eq!(got, expected);
    }

    #[test]
    fn format_fulgor_line_two_alignments() {
        use std::io::Cursor;
        use crate::PseudoAln;
        use super::format_fulgor_line;

        let data = PseudoAln{ones_names: None,  query_id: Some(1262953), ones: Some(vec![0, 1]), query_name: Some("ERR4035126.1262954".to_string()) };

        let expected: Vec<u8> = b"ERR4035126.1262954\t2\t0\t1\n".to_vec();

        let mut got: Vec<u8> = Vec::new();
        format_fulgor_line(&data, &mut got).unwrap();

        assert_eq!(got, expected);
    }

    #[test]
    fn format_fulgor_line_no_alignments() {
        use std::io::Cursor;
        use crate::PseudoAln;
        use super::format_fulgor_line;

        let data = PseudoAln{ones_names: None,  query_id: Some(1262953), ones: Some(vec![]), query_name: Some("ERR4035126.1262954".to_string()) };

        let expected: Vec<u8> = b"ERR4035126.1262954\t0\n".to_vec();

        let mut got: Vec<u8> = Vec::new();
        format_fulgor_line(&data, &mut got).unwrap();

        assert_eq!(got, expected);
    }

    #[test]
    fn error_if_no_query_id() {
        use std::io::Cursor;
        use crate::PseudoAln;
        use super::format_fulgor_line;

        let data = PseudoAln{ones_names: None,  query_id: None, ones: Some(vec![0, 3, 7, 11]), ..Default::default()};

        let got = format_fulgor_line(&data, &mut Vec::new());

        assert!(!got.is_ok());
    }

    #[test]
    fn error_if_no_ones() {
        use std::io::Cursor;
        use crate::PseudoAln;
        use super::format_fulgor_line;

        let data = PseudoAln{ones_names: None,  query_id: Some(128), ones: None, ..Default::default()};

        let got = format_fulgor_line(&data, &mut Vec::new());

        assert!(!got.is_ok());
    }
}
