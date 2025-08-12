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

#[derive(Debug, Clone)]
pub struct BifrostHeaderNotConsumedError;

impl std::fmt::Display for BifrostHeaderNotConsumedError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "Bifrost header not consumed from input `Read`.")
    }
}

impl std::error::Error for BifrostHeaderNotConsumedError {}

/// Parse a line from Bifrost
///
/// Reads a pseudoalignment line stored in the *Bifrost* format.
///
/// If `conn` contains the header line starting with `query_sequence` and
/// listing the reference names, this will consume it and read the first
/// alignment.
///
/// Assumes that no query is named `query_sequence`.
///
/// Returns the [pseudoalignment](PseudoAln) on the line.
///
pub fn read_bifrost<R: Read>(
    conn: &mut R,
) -> Result<PseudoAln, E> {
    let separator: char = '\t';
    let mut contents: String = String::new();
    conn.read_to_string(&mut contents)?;

    let mut records = contents.split(separator);

    let read_name_bytes = records.next().unwrap(); // TODO error if none

    // TODO this comparison doesn't work for some reason
    if read_name_bytes == "query_name" {
        return Err(Box::new(BifrostHeaderNotConsumedError{}))
    }

    let query_name = read_name_bytes.chars().collect::<String>();

    let mut ones: Vec<u32> = Vec::new();
    for (idx, record) in records.enumerate() {
        if record.parse::<u32>().unwrap() > 0 {
            ones.push(idx as u32);
        }
    };

    let res = PseudoAln{ query_id: None, ones, query_name: Some(query_name)};
    Ok(res)
}

// Tests
#[cfg(test)]
mod tests {

    #[test]
    fn read_bifrost_error_if_header_not_consumed() {
        use crate::PseudoAln;
        use super::read_bifrost;
        use std::io::BufRead;
        use std::io::BufReader;
        use std::io::Cursor;

        let mut data: Vec<u8> = b"query_name\tchr.fasta\tplasmid.fasta\n".to_vec();
        data.append(&mut b"ERR4035126.1\t121\t0\n".to_vec());

        let cursor = Cursor::new(data);
        let mut reader = BufReader::new(cursor);
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        let got = read_bifrost(&mut line.as_bytes());

        assert!(!got.is_ok());
    }

    #[test]
    fn read_bifrost_multiple() {
        use crate::PseudoAln;
        use super::read_bifrost;
        use std::io::BufRead;
        use std::io::BufReader;
        use std::io::Cursor;

        let mut data: Vec<u8> = b"query_name\tchr.fasta\tplasmid.fasta\n".to_vec();
        data.append(&mut b"ERR4035126.1\t121\t0\n".to_vec());
        data.append(&mut b"ERR4035126.20\t121\t0\n".to_vec());
        data.append(&mut b"ERR4035126.16\t51\t0\n".to_vec());
        data.append(&mut b"ERR4035126.1262938\t0\t121\n".to_vec());
        data.append(&mut b"ERR4035126.1262940\t121\t0\n".to_vec());
        data.append(&mut b"ERR4035126.1262954\t0\t121\n".to_vec());
        data.append(&mut b"ERR4035126.1262955\t0\t0\n".to_vec());
        data.append(&mut b"ERR4035126.651994\t67\t121\n".to_vec());
        data.append(&mut b"ERR4035126.651993\t121\t121\n".to_vec());
        data.append(&mut b"ERR4035126.1262970\t0\t0\n".to_vec());

        let expected = vec![
            PseudoAln{ query_id: None, ones: vec![0,], query_name: Some("ERR4035126.1".to_string()) },
            PseudoAln{ query_id: None, ones: vec![0], query_name: Some("ERR4035126.20".to_string()) },
            PseudoAln{ query_id: None, ones: vec![0], query_name: Some("ERR4035126.16".to_string()) },
            PseudoAln{ query_id: None, ones: vec![1], query_name: Some("ERR4035126.1262938".to_string()) },
            PseudoAln{ query_id: None, ones: vec![0], query_name: Some("ERR4035126.1262940".to_string()) },
            PseudoAln{ query_id: None, ones: vec![1], query_name: Some("ERR4035126.1262954".to_string()) },
            PseudoAln{ query_id: None, ones: vec![], query_name: Some("ERR4035126.1262955".to_string()) },
            PseudoAln{ query_id: None, ones: vec![0, 1], query_name: Some("ERR4035126.651994".to_string()) },
            PseudoAln{ query_id: None, ones: vec![0, 1], query_name: Some("ERR4035126.651993".to_string()) },
            PseudoAln{ query_id: None, ones: vec![], query_name: Some("ERR4035126.1262970".to_string()) },
        ];

        let cursor = Cursor::new(data);
        let mut reader = BufReader::new(cursor);
        reader.read_line(&mut String::new()).unwrap();
        let got: Vec<PseudoAln> = reader.lines().map(|line| {
            read_bifrost(&mut line.unwrap().as_bytes()).unwrap()
        }).collect();

        assert_eq!(got, expected);
    }
}
