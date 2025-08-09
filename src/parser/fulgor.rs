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

/// Parse a line from Fulgor
///
/// Reads a pseudoalignment line stored in the *Fulgor* format.
///
/// Returns the [pseudoalignment](PseudoAln) on the line.
///
pub fn read_fulgor<R: Read>(
    num_targets: usize,
    conn: &mut R,
) -> Result<PseudoAln, E> {
    let separator: char = '\t';
    let mut contents: String = String::new();
    conn.read_to_string(&mut contents)?;

    let mut records = contents.split(separator);

    let read_name_bytes = records.next().unwrap(); // TODO error if none
    let _ = records.next().unwrap(); // TODO error if none

    // TODO map read_name to query_id outside of this function
    let query_name = read_name_bytes.chars().collect::<String>();

    let mut ones: Vec<bool> = vec![false; num_targets];

    for record in records {
        let id = record.parse::<u32>()?;
        ones[id as usize] = true;
    }

    let res = PseudoAln{ query_id: None, ones, query_name: Some(query_name)};
    Ok(res)
}

// Tests
#[cfg(test)]
mod tests {

    #[test]
    fn read_fulgor_multiple() {
        use crate::PseudoAln;
        use super::read_fulgor;
        use std::io::BufRead;
        use std::io::BufReader;
        use std::io::Cursor;

        let mut data: Vec<u8> = b"ERR4035126.4996\t0\n".to_vec();
        data.append(&mut b"ERR4035126.1262953\t1\t0\n".to_vec());
        data.append(&mut b"ERR4035126.1262954\t1\t1\n".to_vec());
        data.append(&mut b"ERR4035126.1262955\t1\t1\n".to_vec());
        data.append(&mut b"ERR4035126.1262956\t1\t0\n".to_vec());
        data.append(&mut b"ERR4035126.1262957\t1\t0\n".to_vec());
        data.append(&mut b"ERR4035126.1262958\t1\t0\n".to_vec());
        data.append(&mut b"ERR4035126.1262959\t1\t0\n".to_vec());
        data.append(&mut b"ERR4035126.651965\t2\t0\t1\n".to_vec());
        data.append(&mut b"ERR4035126.11302\t0\n".to_vec());
        data.append(&mut b"ERR4035126.1262960\t1\t0\n".to_vec());
        data.append(&mut b"ERR4035126.1262961\t1\t0\n".to_vec());
        data.append(&mut b"ERR4035126.1262962\t1\t0\n".to_vec());
        data.append(&mut b"ERR4035126.651965\t2\t0\t1\n".to_vec());

        let expected = vec![
            PseudoAln{ query_id: Some(4996), ones: vec![false; 2], ..Default::default()},
            PseudoAln{ query_id: Some(126953), ones: vec![true, false], ..Default::default()},
            PseudoAln{ query_id: Some(126954), ones: vec![false, true], ..Default::default()},
            PseudoAln{ query_id: Some(126955), ones: vec![false, true], ..Default::default()},
            PseudoAln{ query_id: Some(126956), ones: vec![true, false], ..Default::default()},
            PseudoAln{ query_id: Some(126957), ones: vec![true, false], ..Default::default()},
            PseudoAln{ query_id: Some(126958), ones: vec![true, false], ..Default::default()},
            PseudoAln{ query_id: Some(126959), ones: vec![true, false], ..Default::default()},
            PseudoAln{ query_id: Some(651965), ones: vec![true, true], ..Default::default()},
            PseudoAln{ query_id: Some(11302), ones: vec![false, false], ..Default::default()},
            PseudoAln{ query_id: Some(1262960), ones: vec![false, true], ..Default::default()},
            PseudoAln{ query_id: Some(1262961), ones: vec![false, true], ..Default::default()},
            PseudoAln{ query_id: Some(1262962), ones: vec![false, true], ..Default::default()},
            PseudoAln{ query_id: Some(651965), ones: vec![false, true], ..Default::default()},
        ];

        let cursor = Cursor::new(data);
        let reader = BufReader::new(cursor);
        let got: Vec<PseudoAln> = reader.lines().map(|line| {
            read_fulgor(2, &mut line.unwrap().as_bytes()).unwrap()
        }).collect();

        assert_eq!(got, expected);
    }
}
