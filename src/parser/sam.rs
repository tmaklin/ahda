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

use noodles_sam as sam;
use noodles_sam::alignment::record::Flags;

use crate::PseudoAln;

type E = Box<dyn std::error::Error>;

/// Parse a line from a [SAM](https://samtools.github.io/hts-specs/SAMv1.pdf) file.
///
/// Reads a pseudoalignment line stored in the *SAM* format.
///
/// Returns the [pseudoalignment](PseudoAln) on the line.
///
pub fn read_sam<R: Read>(
    conn: &mut R,
) -> Result<PseudoAln, E> {
    let mut contents: String = String::new();
    conn.read_to_string(&mut contents)?;

    let record = sam::Record::try_from(contents.as_bytes())?;

    let query_name: String = record.name().unwrap().to_string();

    if record.flags().is_ok() && *record.flags().as_ref().unwrap() == Flags::UNMAPPED {
        return Ok(PseudoAln{query_id: None, ones: None, query_name: Some(query_name), ones_names: None });
    }

    let target: String = record.reference_sequence_name().unwrap().to_string();

    let res = PseudoAln{query_id: None, ones: Some(vec![]), query_name: Some(query_name), ones_names: Some(vec![target]) };
    Ok(res)
}

// Tests
#[cfg(test)]
mod tests {

    #[test]
    fn read_sam_single() {
        use crate::PseudoAln;
        use super::read_sam;
        use std::io::BufRead;
        use std::io::BufReader;
        use std::io::Cursor;

        let data: Vec<u8> =b"ERR4035126.1\t16\tOZ038621.1\t4541508\t60\t151M\t*\t0\t0\tAGTATTTAGTGACCTAAGTCAATAAAATTTTAATTTACTCACGGCAGGTAACCAGTTCAGAAGCTGCTATCAGACACTCTTTTTTTAATCCACACAGAGACATATTGCCCGTTGCAGTCAGAATGAAAAGCTGAAAATCACTTACTAAGGC FJ<<JJFJAA<-JFAJFAF<JFFJJJJJJJFJFJJA<A<AJJAAAFFJJJJFJJFJFJAJJ7JJJJJFJJJJJFFJFFJFJJJJJJFJ7FFJAJJJJJJJJFJJFJJFJFJJJJFJJFJJJJJJJJJFFJJJJJJJJJJJJJFJJJFFAAA\tNM:i:0\tMD:Z:151\tAS:i:151\tXS:i:0\n".to_vec();

        let expected = vec![
            PseudoAln{ones_names: Some(vec!["OZ038621.1".to_string()]), query_id: None, ones: Some(vec![]), query_name: Some("ERR4035126.1".to_string()) },
        ];

        let cursor = Cursor::new(data);
        let reader = BufReader::new(cursor);
        let got: Vec<PseudoAln> = reader.lines().map(|line| {
            read_sam(&mut line.unwrap().as_bytes()).unwrap()
        }).collect();

        assert_eq!(got, expected);
    }
}
