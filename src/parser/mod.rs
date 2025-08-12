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

// Format specific implementations
pub mod themisto;
pub mod fulgor;
pub mod bifrost;

use crate::Format;
use crate::PseudoAln;

use crate::parser::themisto::read_themisto;
use crate::parser::fulgor::read_fulgor;
use crate::parser::bifrost::read_bifrost;

use std::io::BufRead;
use std::io::BufReader;
use std::io::Cursor;
use std::io::Read;

type E = Box<dyn std::error::Error>;

#[derive(Debug, Clone)]
pub struct UnrecognizedInputFormat;

impl std::fmt::Display for UnrecognizedInputFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "Unrecognized input format")
    }
}

impl std::error::Error for UnrecognizedInputFormat {}

#[derive(Debug)]
pub struct Parser<'a, R: Read> {
    // conn: R,
    reader: BufReader<&'a mut R>,
    buf: Cursor<Vec<u8>>,
    pub format: Format,
}

impl<'a, R: Read> Parser<'a, R> {
    pub fn new(
        conn: &'a mut R,
    ) -> Result<Self, E> {
        let mut reader = BufReader::new(conn);
        let mut buf = Cursor::new(Vec::<u8>::new());

        reader.read_until(b'\n', buf.get_mut())?;

        if let Some(format) = guess_format(buf.get_ref()) {
            Ok(Self { reader, buf, format })
        } else {
            Err(Box::new(UnrecognizedInputFormat{}))
        }

    }

}

impl<R: Read> Parser<'_, R> {
    pub fn next(
        &mut self,
    ) -> Option<PseudoAln> {
        // TODO this is a dumb implementation, fix

        let mut line = Cursor::new(Vec::<u8>::new());
        if !self.buf.get_ref().is_empty() {
            line = self.buf.clone();
            if line.get_mut().contains(&b'\n') {
                line.get_mut().pop();
            }
            let res = match self.format {
                Format::Themisto => read_themisto(&mut line).unwrap(),
                Format::Fulgor => read_fulgor(&mut line).unwrap(),
                Format::Bifrost => {
                    let _ = self.read_header();

                    line.get_mut().clear();
                    self.reader.read_until(b'\n', line.get_mut()).unwrap();
                    line.get_mut().pop();
                    read_bifrost(&mut line).unwrap()
                },
            };
            self.buf.get_mut().clear();
            return Some(res)
        }

        if self.reader.read_until(b'\n', line.get_mut()).is_ok() {
            if line.get_mut().is_empty() {
                return None
            }
            line.get_mut().pop();
            let res = match self.format {
                Format::Themisto => read_themisto(&mut line).unwrap(),
                Format::Fulgor => read_fulgor(&mut line).unwrap(),
                Format::Bifrost => read_bifrost(&mut line).unwrap(),
            };
            Some(res)
        } else {
            None
        }
    }

    /// Consumes the header line and returns the target sequence names.
    ///
    /// The header line is only present in Bifrost and Metagraph output. For
    /// Themisto and Fulgor, this will return None.
    ///
    /// Returns None if the header has already been consumed by calling [next].
    pub fn read_header(
        &mut self,
    ) -> Option<Vec<String>> {
        if self.buf.get_ref().is_empty() {
            return None
        }
        match self.format {
            Format::Themisto => None,
            Format::Fulgor => None,
            Format::Bifrost => {
                let separator: char = '\t';
                let contents: String = self.buf.get_ref().iter().map(|x| *x as char).collect();
                let mut records = contents.split(separator);
                // Consume `query_name`
                records.next().unwrap();
                let mut target_names: Vec<String> = Vec::new();
                for record in records {
                    target_names.push(record.to_string());
                }
                let n_targets = target_names.len();
                target_names[n_targets - 1].pop();
                self.buf.get_mut().clear();

                Some(target_names)
            }
        }
    }
}

pub fn guess_format(
    bytes: &[u8],
) -> Option<Format> {
    let first_line: Vec<u8> = if bytes.contains(&b'\n') {
        let linebreak = bytes.iter().position(|x| *x == b'\n').unwrap();
        bytes[0..linebreak].to_vec()
    } else {
        bytes.to_vec()
    };

    let not_themisto: bool = first_line.contains(&b'\t');
    if !not_themisto {
        return Some(Format::Themisto)
    }

    let line = first_line.iter().map(|x| *x as char).collect::<String>();
    let mut records = line.split('\t');

    let bifrost: bool = records.next()? == "query_name";
    if bifrost {
        return Some(Format::Bifrost)
    }

    let next = records.next()?;

    let fulgor: bool = next.parse::<u32>().is_ok();
    if fulgor {
        return Some(Format::Fulgor)
    }

    None
}

// Tests
#[cfg(test)]
mod tests {

    #[test]
    fn guess_format_themisto() {
        use crate::Format;
        use super::guess_format;

        let data: Vec<u8> = b"202678 1\n202728\n651964 0 1\n651966 0 1\n1166624 0\n1166625 0\n1166626 1".to_vec();
        let got = guess_format(&data).unwrap();
        let expected = Format::Themisto;

        assert_eq!(got, expected);
    }

    #[test]
    fn guess_format_fulgor() {
        use crate::Format;
        use super::guess_format;

        let mut data: Vec<u8> = b"ERR4035126.4996\t0\n".to_vec();
        data.append(&mut b"ERR4035126.1262953\t1\t0\n".to_vec());
        data.append(&mut b"ERR4035126.1262954\t1\t1\n".to_vec());

        let got = guess_format(&data).unwrap();
        let expected = Format::Fulgor;

        assert_eq!(got, expected);
    }

    #[test]
    fn guess_format_bifrost() {
        use crate::Format;
        use super::guess_format;

        let mut data: Vec<u8> = b"query_name\tchromosome.fasta\tplasmid.fasta\n".to_vec();
        data.append(&mut b"ERR4035126.1262953\t1\t0\t15\n".to_vec());
        data.append(&mut b"ERR4035126.1262954\t1\t1\t0\n".to_vec());

        let got = guess_format(&data).unwrap();
        let expected = Format::Bifrost;

        assert_eq!(got, expected);
    }

    #[test]
    fn consume_bifrost_header_with_next() {
        use super::Parser;
        use crate::PseudoAln;
        use std::io::Cursor;

        let mut data: Vec<u8> = b"query_name\tchr.fasta\tplasmid.fasta\n".to_vec();
        data.append(&mut b"ERR4035126.1\t121\t0\n".to_vec());
        let expected: PseudoAln = PseudoAln{ query_id: None, ones: vec![0], query_name: Some("ERR4035126.1".to_string()) };

        let mut cursor = Cursor::new(data);

        let mut reader = Parser::new(&mut cursor).unwrap();

        let got: PseudoAln = reader.next().unwrap();

        assert_eq!(got, expected);
    }

    #[test]
    fn read_bifrost_header() {
        use super::Parser;
        use crate::PseudoAln;
        use std::io::Cursor;

        let mut data: Vec<u8> = b"query_name\tchr.fasta\tplasmid.fasta\n".to_vec();
        data.append(&mut b"ERR4035126.1\t121\t0\n".to_vec());
        let expected: Vec<String> = vec!["chr.fasta".to_string(), "plasmid.fasta".to_string()];

        let mut cursor = Cursor::new(data);

        let mut reader = Parser::new(&mut cursor).unwrap();

        let got: Vec<String> = reader.read_header().unwrap();

        assert_eq!(got, expected);
    }
}
