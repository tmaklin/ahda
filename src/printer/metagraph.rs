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
pub struct MetagraphPrinterError;

impl std::fmt::Display for MetagraphPrinterError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "invalid input to encode")
    }
}

impl std::error::Error for MetagraphPrinterError {}

/// Format a single pseudoalignment in Metagraph format
///
/// Writes bytes containing the formatted line containing the contents of
/// `aln` to `conn`.
///
/// Terminates with a [MetagraphPrinterError] if [PseudoAln::query_id] or
/// [PseudoAln::ones] is None.
///
pub fn format_metagraph_line<W: Write>(
    aln: &PseudoAln,
    conn: &mut W,
) -> Result<(), E> {
    let separator: char = '\t';
    let mut formatted: String = String::new();

    if aln.ones_names.is_none() || aln.query_name.is_none() {
        return Err(Box::new(MetagraphPrinterError{}))
    }

    formatted += &aln.query_id.as_ref().unwrap().to_string();
    formatted += &separator.to_string();

    formatted += &aln.query_name.clone().unwrap().to_string();
    formatted += &separator.to_string();

    aln.ones_names.as_ref().unwrap().iter().for_each(|name| {
        formatted += name;
        formatted += &':'.to_string();

    });
    if !aln.ones_names.as_ref().unwrap().is_empty() {
        formatted.pop();
    }
    formatted += "\n";

    conn.write_all(formatted.as_bytes())?;
    Ok(())
}

// Tests
#[cfg(test)]
mod tests {

    #[test]
    fn format_metagraph_line_one_aligned() {
        use crate::PseudoAln;
        use super::format_metagraph_line;

        let data = PseudoAln{ones_names: Some(vec!["chr.fasta".to_string()]), query_id: None, ones: None, query_name: Some("ERR4035126.1262940".to_string()) };

        let expected: Vec<u8> = b"ERR4035126.1262940\tchr.fasta\n".to_vec();

        let mut got: Vec<u8> = Vec::new();
        format_metagraph_line(&data, &mut got).unwrap();

        assert_eq!(got, expected)
    }

    #[test]
    fn format_metagraph_line_two_aligned() {
        use crate::PseudoAln;
        use super::format_metagraph_line;

        let data = PseudoAln{ones_names: Some(vec!["chr.fasta".to_string(), "plasmid.fasta".to_string()]), query_id: None, ones: None, query_name: Some("ERR4035126.1262940".to_string()) };

        let expected: Vec<u8> = b"ERR4035126.1262940\tchr.fasta:plasmid.fasta\n".to_vec();

        let mut got: Vec<u8> = Vec::new();
        format_metagraph_line(&data, &mut got).unwrap();

        assert_eq!(got, expected)
    }

    #[test]
    fn format_metagraph_line_none_aligned() {
        use crate::PseudoAln;
        use super::format_metagraph_line;

        let data = PseudoAln{ones_names: Some(vec![]), query_id: None, ones: None, query_name: Some("ERR4035126.1262940".to_string()) };

        let expected: Vec<u8> = b"ERR4035126.1262940\t\n".to_vec();

        let mut got: Vec<u8> = Vec::new();
        format_metagraph_line(&data, &mut got).unwrap();

        assert_eq!(got, expected)
    }

    #[test]
    fn line_error_if_no_query_name() {
        use crate::PseudoAln;
        use super::format_metagraph_line;

        let data = PseudoAln{ones_names: Some(vec!["chr.fasta".to_string()]), query_name: None, ones: None, query_id: None};

        let got = format_metagraph_line(&data, &mut Vec::new());

        assert!(!got.is_ok());
    }

    #[test]
    fn line_error_if_no_ones_names() {
        use crate::PseudoAln;
        use super::format_metagraph_line;

        let data = PseudoAln{ones_names: None, query_name: Some("ERR4035126.1262954".to_string()), query_id: Some(128), ones: None};

        let got = format_metagraph_line(&data, &mut Vec::new());

        assert!(!got.is_ok());
    }
}
