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

//! Ahda .tsv `query` parser.
//!
//! ## Expected format
//! TODO What was this generated with
//! This example was generated with bifrost v1.3.5 from the `bifrost query` subcommand.
//!
//! ```text
//! query_index    query_name      chromosome.fasta        plasmid.fasta
//! 0    FastqQuery.1    0    0
//! 1    FastqQuery.2    0    0
//! 2    FastqQuery.3    1    0
//! 135608    FastqQuery.135609    1    1
//! 100818    FastqQuery.100819    0    1
//! ```
//!
//! or, with tabs and line breaks visible:
//! ```text
//! query_index\tquery_name\tchromosome.fasta\tplasmid.fasta\n
//! 0\tFastqQuery.1\t0\t0\n
//! 1\tFastqQuery.2\t0\t0\n
//! 2\tFastqQuery.3\t1\t0\n
//! 135608\tFastqQuery.135609\t1\t1\n
//! 100818\tFastqQuery.100819\t0\t1\n
//! ```
//!
//! ### Pros of the ahda .tsv format
//! - The first colum contains the index of the query sequence.
//! - The second column contains the name of the query sequence.
//! - The subsequent columns contain 0 for no alignment or a value >= 1 for alignment against each target sequence.
//! - Queries with no alignments are shown.
//! - Number of queries can be inferred from the file.
//! - Number of target sequences can be inferred from the header.
//! - Names of the target sequences can be inferred from the header.
//! - Indexes of the target sequences can be inferred from the header.
//!
//! ### Cons of the ahda .tsv format
//! - Space inefficient if the number of target sequences and queries is large.
//!
use std::io::Read;

use crate::PseudoAln;
use crate::errors::CorruptedInputErr;

type E = Box<dyn std::error::Error>;

/// Parse a line from ahda .tsv
///
/// Reads a pseudoalignment line stored in the *ahda .tsv* format.
///
/// If `conn` contains the header line starting with `query_index    query_name` and
/// listing the reference names, this will consume it and read the first
/// alignment.
///
/// Assumes that no query is named `query_index    query_name`.
///
/// Returns the [pseudoalignment](PseudoAln) on the line.
///
pub fn read_ahda_tsv<R: Read>(
    conn: &mut R,
) -> Result<PseudoAln, E> {
    let separator: char = '\t';
    let mut contents: String = String::new();
    conn.read_to_string(&mut contents)?;

    let mut records = contents.split(separator);

    let bytes = records.next().ok_or(CorruptedInputErr)?;
    if bytes == "query_index" {
        return Err(Box::new(crate::errors::AhdaTSVHeaderNotConsumedError{}))
    }
    let query_index = bytes.parse::<u32>()?;

    let bytes = records.next().ok_or(CorruptedInputErr)?;
    if bytes == "query_name" {
        return Err(Box::new(crate::errors::AhdaTSVHeaderNotConsumedError{}))
    }
    let query_name = bytes.as_bytes().to_vec();

    let mut ones: Vec<u32> = Vec::new();
    for (idx, record) in records.enumerate() {
        if record.parse::<u32>().unwrap() > 0 {
            ones.push(idx as u32);
        }
    };

    let res = PseudoAln{ones_names: None,  query_id: Some(query_index), ones: Some(ones), query_name: Some(query_name)};
    Ok(res)
}

// Tests
#[cfg(test)]
mod tests {

    #[test]
    fn read_ahda_tsv_error_if_header_not_consumed() {
        use super::read_ahda_tsv;
        use std::io::BufRead;
        use std::io::BufReader;
        use std::io::Cursor;

        let mut data: Vec<u8> = b"query_index\tquery_name\tchromosome.fasta\tplasmid.fasta\n".to_vec();
        data.append(&mut b"0\tFastqQuery.1\t0\t0\n".to_vec());
        data.append(&mut b"1\tFastqQuery.2\t0\t0\n".to_vec());
        data.append(&mut b"2\tFastqQuery.3\t1\t0\n".to_vec());
        data.append(&mut b"135608\tFastqQuery.135609\t1\t1\n".to_vec());
        data.append(&mut b"100818\tFastqQuery.100819\t0\t1\n".to_vec());

        let cursor = Cursor::new(data);
        let mut reader = BufReader::new(cursor);
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        let got = read_ahda_tsv(&mut line.as_bytes());

        assert!(got.is_err());
    }

    #[test]
    fn read_ahda_tsv_multiple() {
        use crate::PseudoAln;
        use super::read_ahda_tsv;
        use std::io::BufRead;
        use std::io::BufReader;
        use std::io::Cursor;

        let mut data: Vec<u8> = b"query_index\tquery_name\tchromosome.fasta\tplasmid.fasta\n".to_vec();
        data.append(&mut b"0\tFastqQuery.1\t0\t0\n".to_vec());
        data.append(&mut b"1\tFastqQuery.2\t0\t0\n".to_vec());
        data.append(&mut b"2\tFastqQuery.3\t1\t0\n".to_vec());
        data.append(&mut b"135608\tFastqQuery.135609\t1\t1\n".to_vec());
        data.append(&mut b"100818\tFastqQuery.100819\t0\t1\n".to_vec());

        let expected = vec![
            PseudoAln{ones_names: None,  query_id: Some(0), ones: Some(vec![]), query_name: Some("FastqQuery.1".as_bytes().to_vec()) },
            PseudoAln{ones_names: None,  query_id: Some(1), ones: Some(vec![]), query_name: Some("FastqQuery.2".as_bytes().to_vec()) },
            PseudoAln{ones_names: None,  query_id: Some(2), ones: Some(vec![0]), query_name: Some("FastqQuery.3".as_bytes().to_vec()) },
            PseudoAln{ones_names: None,  query_id: Some(135608), ones: Some(vec![0, 1]), query_name: Some("FastqQuery.135609".as_bytes().to_vec()) },
            PseudoAln{ones_names: None,  query_id: Some(100818), ones: Some(vec![1]), query_name: Some("FastqQuery.100819".as_bytes().to_vec()) },
        ];

        let cursor = Cursor::new(data);
        let mut reader = BufReader::new(cursor);
        reader.read_line(&mut String::new()).unwrap();
        let got: Vec<PseudoAln> = reader.lines().map(|line| {
            read_ahda_tsv(&mut line.unwrap().as_bytes()).unwrap()
        }).collect();

        assert_eq!(got, expected);
    }
}
