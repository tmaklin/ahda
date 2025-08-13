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
pub struct BifrostPrinterError;

impl std::fmt::Display for BifrostPrinterError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "invalid input to encode")
    }
}

impl std::error::Error for BifrostPrinterError {}

/// Format a single pseudoalignment in Bifrost format
///
/// Writes bytes containing the formatted line containing the contents of
/// `aln` to `conn`.
///
/// Terminates with a [BifrostPrinterError] if [PseudoAln::query_id] or
/// [PseudoAln::ones] is None.
///
pub fn format_bifrost_line<W: Write>(
    aln: &PseudoAln,
    n_targets: usize,
    conn: &mut W,
) -> Result<(), E> {
    let separator: char = '\t';
    let mut formatted: String = String::new();

    if aln.ones.is_none() || aln.query_name.is_none() {
        return Err(Box::new(BifrostPrinterError{}))
    }

    formatted += &aln.query_name.clone().unwrap().to_string();

    let ones: &Vec<u32> = aln.ones.as_ref().unwrap();
    let mut ones_bits: Vec<bool> = vec![false; n_targets];
    ones.iter().for_each(|is_set_idx| ones_bits[*is_set_idx as usize] = true);

    ones_bits.iter().for_each(|is_set| {
        formatted += &separator.to_string();
            formatted += &(*is_set as u32).to_string();

    });
    formatted += "\n";

    conn.write_all(formatted.as_bytes())?;
    Ok(())
}

// Tests
#[cfg(test)]
mod tests {

    #[test]
    fn format_bifrost_line_1st_aligned() {
        use crate::PseudoAln;
        use super::format_bifrost_line;

        let data = PseudoAln{ones_names: None, query_id: None, ones: Some(vec![0]), query_name: Some("ERR4035126.1262940".to_string()) };

        let expected: Vec<u8> = b"ERR4035126.1262940\t1\t0\n".to_vec();

        let mut got: Vec<u8> = Vec::new();
        format_bifrost_line(&data, 2, &mut got).unwrap();

        assert_eq!(got, expected)
    }

    #[test]
    fn format_bifrost_line_2nd_aligned() {
        use crate::PseudoAln;
        use super::format_bifrost_line;

        let data = PseudoAln{ones_names: None, query_id: None, ones: Some(vec![1]), query_name: Some("ERR4035126.1262940".to_string()) };

        let expected: Vec<u8> = b"ERR4035126.1262940\t0\t1\n".to_vec();

        let mut got: Vec<u8> = Vec::new();
        format_bifrost_line(&data, 2, &mut got).unwrap();

        assert_eq!(got, expected)
    }

    #[test]
    fn format_bifrost_line_both_aligned() {
        use crate::PseudoAln;
        use super::format_bifrost_line;

        let data = PseudoAln{ones_names: None, query_id: None, ones: Some(vec![0,1]), query_name: Some("ERR4035126.1262940".to_string()) };

        let expected: Vec<u8> = b"ERR4035126.1262940\t1\t1\n".to_vec();

        let mut got: Vec<u8> = Vec::new();
        format_bifrost_line(&data, 2, &mut got).unwrap();

        assert_eq!(got, expected)
    }

    #[test]
    fn format_bifrost_line_no_alignments() {
        use crate::PseudoAln;
        use super::format_bifrost_line;

        let data = PseudoAln{ones_names: None, query_id: None, ones: Some(vec![]), query_name: Some("ERR4035126.1262940".to_string()) };

        let expected: Vec<u8> = b"ERR4035126.1262940\t0\t0\n".to_vec();

        let mut got: Vec<u8> = Vec::new();
        format_bifrost_line(&data, 2, &mut got).unwrap();

        assert_eq!(got, expected)
    }

    #[test]
    fn error_if_no_query_name() {
        use std::io::Cursor;
        use crate::PseudoAln;
        use super::format_bifrost_line;

        let data = PseudoAln{ones_names: None, query_name: None, ones: Some(vec![0, 3, 7, 11]), query_id: None};

        let got = format_bifrost_line(&data, 2, &mut Vec::new());

        assert!(!got.is_ok());
    }

    #[test]
    fn error_if_no_ones() {
        use std::io::Cursor;
        use crate::PseudoAln;
        use super::format_bifrost_line;

        let data = PseudoAln{ones_names: None, query_name: Some("ERR4035126.1262954".to_string()), query_id: Some(128), ones: None};

        let got = format_bifrost_line(&data, 2, &mut Vec::new());

        assert!(!got.is_ok());
    }
}
