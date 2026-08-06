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

/// Format a single pseudoalignment in Metagraph format
///
/// Writes bytes containing the formatted line containing the contents of
/// `aln` to `conn`.
///
/// Terminates with a [MetagraphPrinterError](crate::errors::MetagraphPrinterError)
/// if the `query_id` field of [PseudoAln] or the `ones` field
/// of [PseudoAln] is None.
///
pub fn format_metagraph_line<W: Write>(
    aln: &PseudoAln,
    conn: &mut W,
) -> Result<(), E> {
    let separator: char = '\t';
    let mut formatted: String = String::new();

    if let Some(q_id) = &aln.query_id {
        formatted += &q_id.to_string();
        formatted += &separator.to_string();
    } else {
        return Err(Box::new(crate::errors::PseudoAlnQueryIdIsEmpty{}))
    };

    if let Some(name) = &aln.query_name {
        let q_name = String::from_utf8(name.clone())?;
        formatted += &q_name;
        formatted += &separator.to_string();
    } else {
        return Err(Box::new(crate::errors::PseudoAlnQueryNameIsEmpty{}))
    };

    if let Some(ones_names) = &aln.ones_names {
        for name in ones_names {
            let target_name = String::from_utf8(name.clone())?;
            formatted += &target_name;
            formatted += &':'.to_string();

        }
        if !ones_names.is_empty() {
            formatted.pop();
        }
        formatted += "\n";
    } else {
        return Err(Box::new(crate::errors::PseudoAlnOnesNamesIsEmpty{}))
    }

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

        let data = PseudoAln{ones_names: Some(vec!["chr.fasta".as_bytes().to_vec()]), query_id: Some(0), ones: None, query_name: Some("ERR4035126.1262940".as_bytes().to_vec()) };

        let expected: Vec<u8> = b"0\tERR4035126.1262940\tchr.fasta\n".to_vec();

        let mut got: Vec<u8> = Vec::new();
        format_metagraph_line(&data, &mut got).unwrap();

        assert_eq!(got, expected)
    }

    #[test]
    fn format_metagraph_line_two_aligned() {
        use crate::PseudoAln;
        use super::format_metagraph_line;

        let data = PseudoAln{ones_names: Some(vec!["chr.fasta".as_bytes().to_vec(), "plasmid.fasta".as_bytes().to_vec()]), query_id: Some(0), ones: None, query_name: Some("ERR4035126.1262940".as_bytes().to_vec()) };

        let expected: Vec<u8> = b"0\tERR4035126.1262940\tchr.fasta:plasmid.fasta\n".to_vec();

        let mut got: Vec<u8> = Vec::new();
        format_metagraph_line(&data, &mut got).unwrap();

        assert_eq!(got, expected)
    }

    #[test]
    fn format_metagraph_line_none_aligned() {
        use crate::PseudoAln;
        use super::format_metagraph_line;

        let data = PseudoAln{ones_names: Some(vec![]), query_id: Some(0), ones: None, query_name: Some("ERR4035126.1262940".as_bytes().to_vec()) };

        let expected: Vec<u8> = b"0\tERR4035126.1262940\t\n".to_vec();

        let mut got: Vec<u8> = Vec::new();
        format_metagraph_line(&data, &mut got).unwrap();

        assert_eq!(got, expected)
    }

    #[test]
    fn line_error_if_no_query_name() {
        use crate::PseudoAln;
        use super::format_metagraph_line;

        let data = PseudoAln{ones_names: Some(vec!["chr.fasta".as_bytes().to_vec()]), query_name: None, ones: None, query_id: None};

        let got = format_metagraph_line(&data, &mut Vec::new());

        assert!(!got.is_ok());
    }

    #[test]
    fn line_error_if_no_ones_names() {
        use crate::PseudoAln;
        use super::format_metagraph_line;

        let data = PseudoAln{ones_names: None, query_name: Some("ERR4035126.1262954".as_bytes().to_vec()), query_id: Some(128), ones: None};

        let got = format_metagraph_line(&data, &mut Vec::new());

        assert!(!got.is_ok());
    }

    #[test]
    fn line_error_if_no_query_id() {
        use crate::PseudoAln;
        use super::format_metagraph_line;

        let data = PseudoAln{ones_names:  Some(vec!["chr.fasta".as_bytes().to_vec()]), query_name: Some("ERR4035126.1262954".as_bytes().to_vec()), query_id: None, ones: None};

        let got = format_metagraph_line(&data, &mut Vec::new());

        assert!(!got.is_ok());
    }
}
