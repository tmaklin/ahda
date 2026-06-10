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

//! Metagraph `--query-mode labels` parser.
//!
//! ahda supports input format produced by `metagraph query --query-mode labels`.
//!
//! ## Expected format
//! This example was generated with metagraph v0.5.2.
//!
//! ```text
//! 0    FastqQuery.1
//! 1    FastqQuery.2
//! 3    FastqQuery.4    chromosome.fasta
//! 9    FastqQuery.10
//! 202728    FastqQuery.202729    chromosome.fasta:plasmid.fasta
//! 7542    FastqQuery.7543    plasmid.fasta
//! ```
//!
//! or, with tabs and line breaks visible:
//! ```text
//! 0\tFastqQuery.1\t$
//! 1\tFastqQuery.2\t$
//! 3\tFastqQuery.4\tchromosome.fasta$
//! 9\tFastqQuery.10\t$
//! 202728\tFastqQuery.202729\tchromosome.fasta:plasmid.fasta$
//! 7542\tFastqQuery.7543\tplasmid.fasta$
//! ```
//!
//! ### Pros of the metagraph format
//! - The first column contains the index of the query in the input .fastx file.
//! - The second column contains the name of the query sequence.
//! - The third column contains names of the aligned target sequences.
//! - Queries with no alignments are shown.
//! - Number of queries can be inferred from the file.
//!
//! ### Cons of the metagraph format
//! - Number of target sequences cannot be inferred with certainty.
//! - Index of the target sequence is not given.
//!
//! ### Other considerations for the metagraph format
//! - Different `--query-mode`s produce different output formats.
//!

use std::io::Read;

use crate::PseudoAln;

type E = Box<dyn std::error::Error>;

/// Parse a line from Metagraph
///
/// Reads a pseudoalignment line stored in the *Metagraph* format.
///
/// Returns the [pseudoalignment](PseudoAln) on the line.
///
pub fn read_metagraph<R: Read>(
    conn: &mut R,
) -> Result<PseudoAln, E> {
    let separator: char = '\t';
    let mut contents: String = String::new();
    conn.read_to_string(&mut contents)?;

    let mut records = contents.split(separator);

    let query_id: u32 = records.next().unwrap().parse::<u32>().unwrap();
    let query_name = records.next().unwrap().as_bytes().to_vec();

    let mut ones_names: Vec<String> = Vec::new();

    let ones_records = records.next().unwrap().split(':');

    for record in ones_records {
        if !record.is_empty() {
            ones_names.push(record.to_string());
        }
    };

    let res = PseudoAln{ ones_names: Some(ones_names),  query_id: Some(query_id), ones: None, query_name: Some(query_name)};
    Ok(res)
}

// Tests
#[cfg(test)]
mod tests {

    #[test]
    fn read_metagraph_multiple() {
        use crate::PseudoAln;
        use super::read_metagraph;
        use std::io::BufRead;
        use std::io::BufReader;
        use std::io::Cursor;

        let mut data: Vec<u8> = b"3\tERR4035126.2\tchr.fasta\n".to_vec();
        data.append(&mut b"2\tERR4035126.1\tchr.fasta\n".to_vec());
        data.append(&mut b"1303804\tERR4035126.651903\tchr.fasta:plasmid.fasta\n".to_vec());
        data.append(&mut b"30\tERR4035126.16\t\n".to_vec());
        data.append(&mut b"15084\tERR4035126.7543\tplasmid.fasta\n".to_vec());

        let expected = vec![
            PseudoAln{ones_names: Some(vec!["chr.fasta".to_string()]),  query_id: Some(3), ones: None, query_name: Some("ERR4035126.2".to_string()) },
            PseudoAln{ones_names: Some(vec!["chr.fasta".to_string()]),  query_id: Some(2), ones: None, query_name: Some("ERR4035126.1".to_string()) },
            PseudoAln{ones_names: Some(vec!["chr.fasta".to_string(), "plasmid.fasta".to_string()]),  query_id: Some(1303804), ones: None, query_name: Some("ERR4035126.651903".to_string()) },
            PseudoAln{ones_names: Some(vec![]),  query_id: Some(30), ones: None, query_name: Some("ERR4035126.16".to_string()) },
            PseudoAln{ones_names: Some(vec!["plasmid.fasta".to_string()]),  query_id: Some(15084), ones: None, query_name: Some("ERR4035126.7543".to_string()) },
        ];

        let cursor = Cursor::new(data);
        let reader = BufReader::new(cursor);
        let got: Vec<PseudoAln> = reader.lines().map(|line| {
            read_metagraph(&mut line.unwrap().as_bytes()).unwrap()
        }).collect();

        assert_eq!(got, expected);
    }
}
