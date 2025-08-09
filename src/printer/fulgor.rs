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

/// Format a single pseudoalignment in Fulgor format
///
/// Writes bytes containing the formatted line containing the contents of
/// `aln` to `conn`.
///
pub fn format_fulgor_line<W: Write>(
    aln: &PseudoAln,
    conn: &mut W,
) -> Result<(), E> {
    let separator: char = '\t';
    let mut formatted: String = String::new();

    // TODO error if query_name is None
    formatted += &aln.query_name.clone().unwrap().to_string();
    formatted += &separator.to_string();
    let aln_count_pos = formatted.len();
    formatted += &0.to_string();
    formatted += &separator.to_string();

    aln.ones.iter().enumerate().for_each(|(idx, aligned)| {
        if *aligned {
            formatted += &separator.to_string();
            formatted += &idx.to_string();
        }
    });
    formatted += "\n";

    conn.write_all(formatted.as_bytes())?;
    Ok(())
}

/// Format many pseudoalignments in Fulgor format
///
/// Writes bytes containing the formatted line containing the contents of
/// `alns` to `conn`.
///
pub fn format_fulgor_file<W: Write>(
    alns: &[PseudoAln],
    conn: &mut W,
) -> Result<(), E> {
    for aln in alns {
        format_fulgor_line(aln, conn)?;
    }
    conn.flush()?;
    Ok(())
}

// Tests
#[cfg(test)]
mod tests {

    // #[test]
    // fn format_themisto_line() {
    //     use std::io::Cursor;
    //     use crate::PseudoAln;
    //     use super::format_themisto_line;

    //     let data = PseudoAln{ query_id: Some(128), ones: vec![true, false, false, true, false, false, false, true, false, false, false, true], ..Default::default()};
    //     let expected: Vec<u8> = vec![49, 50, 56, 32, 48, 32, 51, 32, 55, 32, 49, 49, 10];

    //     let mut got: Vec<u8> = Vec::new();
    //     format_themisto_line(&data, &mut got).unwrap();

    //     assert_eq!(got, expected);
    // }
}
