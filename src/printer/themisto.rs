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
pub struct ThemistoPrinterError;

impl std::fmt::Display for ThemistoPrinterError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "invalid input to encode")
    }
}

impl std::error::Error for ThemistoPrinterError {}

/// Format a single pseudoalignment in Themisto format
///
/// Writes bytes containing the formatted line containing the contents of
/// `aln` to `conn`.
///
/// Terminates with a [ThemistoPrinterError] if [PseudoAln::query_id] or
/// [PseudoAln::ones] is None.
///
pub fn format_themisto_line<W: Write>(
    aln: &PseudoAln,
    conn: &mut W,
) -> Result<(), E> {
    let separator: char = ' ';
    let mut formatted: String = String::new();

    if aln.ones.is_none() || aln.query_id.is_none() {
        return Err(Box::new(ThemistoPrinterError{}))
    }

    formatted += &aln.query_id.unwrap().to_string();

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
    fn format_themisto_line_single_alignment() {
        use std::io::Cursor;
        use crate::PseudoAln;
        use super::format_themisto_line;

        let data = PseudoAln{ones_names: None,  query_id: Some(128), ones: Some(vec![0]), ..Default::default()};
        let expected: Vec<u8> = b"128 0\n".to_vec();

        let mut got: Vec<u8> = Vec::new();
        format_themisto_line(&data, &mut got).unwrap();

        assert_eq!(got, expected);
    }

    #[test]
    fn format_themisto_line_many_alignments() {
        use std::io::Cursor;
        use crate::PseudoAln;
        use super::format_themisto_line;

        let data = PseudoAln{ones_names: None,  query_id: Some(128), ones: Some(vec![0, 3, 7, 11]), ..Default::default()};
        let expected: Vec<u8> = b"128 0 3 7 11\n".to_vec();

        let mut got: Vec<u8> = Vec::new();
        format_themisto_line(&data, &mut got).unwrap();

        assert_eq!(got, expected);
    }

    fn format_themisto_line_no_alignments() {
        use std::io::Cursor;
        use crate::PseudoAln;
        use super::format_themisto_line;

        let data = PseudoAln{ones_names: None,  query_id: Some(128), ones: Some(vec![]), ..Default::default()};
        let expected: Vec<u8> = b"128\n".to_vec();

        let mut got: Vec<u8> = Vec::new();
        format_themisto_line(&data, &mut got).unwrap();

        assert_eq!(got, expected);
    }

    #[test]
    fn error_if_no_query_id() {
        use std::io::Cursor;
        use crate::PseudoAln;
        use super::format_themisto_line;

        let data = PseudoAln{ones_names: None,  query_id: None, ones: Some(vec![0, 3, 7, 11]), ..Default::default()};

        let got = format_themisto_line(&data, &mut Vec::new());

        assert!(!got.is_ok());
    }

    #[test]
    fn error_if_no_ones() {
        use std::io::Cursor;
        use crate::PseudoAln;
        use super::format_themisto_line;

        let data = PseudoAln{ones_names: None,  query_id: Some(128), ones: None, ..Default::default()};

        let got = format_themisto_line(&data, &mut Vec::new());

        assert!(!got.is_ok());
    }
}
