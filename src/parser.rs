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

/// Parse a line from Themisto
///
/// Reads a pseudoalignment line stored in the *Themisto* format.
///
/// Returns the number of pseudoalignments on the line.
///
pub fn read_themisto<R: Read>(
    num_targets: usize,
    conn: &mut R,
) -> Result<PseudoAln, E> {
    let separator: char = ' ';
    let mut contents: String = String::new();
    conn.read_to_string(&mut contents)?;
    eprintln!("{:?}", contents);

    let mut records = contents.split(separator);

    let id_bytes = records.next().unwrap(); // TODO handle empty input
    let read_id = id_bytes.parse::<u32>()?;

    let mut ones: Vec<bool> = vec![false; num_targets];

    for record in records {
        let id = record.parse::<u32>()?;
        ones[id as usize] = true;
    }

    let res = PseudoAln{ read_id, ones };
    Ok(res)
}

// Tests
#[cfg(test)]
mod tests {

    #[test]
    fn read_themisto() {
        use std::io::Cursor;
        use crate::PseudoAln;
        use super::read_themisto;

        let data: Vec<u8> = b"128 0 7 11 3".to_vec();
        let expected = PseudoAln{ read_id: 128, ones: vec![true, false, false, true, false, false, false, true, false, false, false, true] };

        let mut input: Cursor<Vec<u8>> = Cursor::new(data);
        let got = read_themisto(12, &mut input).unwrap();

        assert_eq!(got, expected);
    }
}
