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

//! Parser for reading plain text data into memory from [Read].
//!
//! Reads in 1 [PseudoAln](ahda::PseudoAln) at a time using next(), in the order
//! they appear in the input.
//!
//! The input format is detected automatically based on rules in [guess_format].
//! Some input formats may be ambiguous, in which case the format needs to be
//! specified using [new_with_format](Parser::new_with_format).
//!
//! If the input format includes header data, this will be consumed by Parser on
//! when next() is called for the first time.
//!
//! ## Usage
//!
//! Read in plain text data
//!
//! ```rust
//! use ahda::parser::Parser;
//! use ahda::{decode_from_read, PseudoAln};
//! use std::io::{Cursor, Seek, Write};
//!
//! // Mock inputs that will be stored in FileHeader and FileFlags
//! let targets = vec!["chr.fasta".as_bytes().to_vec(), "plasmid.fasta".as_bytes().to_vec(), "virus.fasta".as_bytes().to_vec()];
//! let queries = vec![b"r1".to_vec(), b"r2".to_vec(), b"r651903".to_vec(), b"r7543".to_vec(), b"r16".to_vec()];
//! let name = "sample".to_string();
//!
//! // Have this plain text:
//! //   3    r7543    chr.fasta:virus.fasta
//! //   0    r1       virus.fasta
//! //   4    r16      chr.fasta:plasmid.fasta:virus.fasta
//! //   2    r651903
//! //
//! let mut plaintext: Vec<u8> = Vec::new();
//! plaintext.append(&mut b"0\tr1\tvirus.fasta\n".to_vec());
//! plaintext.append(&mut b"3\tr7543\tchr.fasta:virus.fasta\n".to_vec());
//! plaintext.append(&mut b"4\tr16\tchr.fasta:plasmid.fasta:virus.fasta\n".to_vec());
//! plaintext.append(&mut b"2\tr651903\t\n".to_vec());
//!
//! let mut input: Cursor<Vec<u8>> = Cursor::new(plaintext.clone());
//!
//! // Create a Parser to convert the plain text data to PseudoAlns
//! let mut it = queries.into_iter();
//! let mut t_it = targets.into_iter();
//! let mut parser = Parser::new(&mut input, Some(&mut it), Some(&mut t_it)).unwrap();
//!
//! let mut alns: Vec<PseudoAln> = Vec::new();
//!
//! // Push all records to `alns`
//! for record in parser.by_ref() {
//!     alns.push(record);
//! }
//!
//! assert_eq!(alns[1], PseudoAln { ones: Some(vec![0, 2]), ones_names: Some(vec!["chr.fasta".as_bytes().to_vec(), "virus.fasta".as_bytes().to_vec()]), query_id: Some(3), query_name: Some("r7543".as_bytes().to_vec()) });
//! assert_eq!(alns[0], PseudoAln { ones: Some(vec![2]), ones_names: Some(vec!["virus.fasta".as_bytes().to_vec()]), query_id: Some(0), query_name: Some("r1".as_bytes().to_vec()) });
//! assert_eq!(alns[2], PseudoAln { ones: Some(vec![0, 1, 2]), ones_names: Some(vec!["chr.fasta".as_bytes().to_vec(), "plasmid.fasta".as_bytes().to_vec(), "virus.fasta".as_bytes().to_vec()]), query_id: Some(4), query_name: Some("r16".as_bytes().to_vec()) });
//! assert_eq!(alns[3], PseudoAln { ones: Some(vec![]), ones_names: Some(vec![]), query_id: Some(2), query_name: Some("r651903".as_bytes().to_vec()) });
//! assert_eq!(alns.len(), 4);
//! ```

// Format specific implementations
pub mod bifrost;
pub mod fulgor;
pub mod metagraph;
pub mod sam;
pub mod themisto;

use crate::Format;
use crate::PseudoAln;

use crate::parser::bifrost::read_bifrost;
use crate::parser::fulgor::read_fulgor;
use crate::parser::metagraph::read_metagraph;
use crate::parser::sam::read_sam;
use crate::parser::themisto::read_themisto;

use indexmap::IndexSet;

use std::io::BufRead;
use std::io::BufReader;
use std::io::Cursor;
use std::io::Seek;
use std::io::Read;

type E = Box<dyn std::error::Error>;

pub struct Parser<'a, R: Read> {
    reader: BufReader<&'a mut R>,
    buf: Cursor<Vec<u8>>,
    pub format: Format,

    query_to_pos: IndexSet<Vec<u8>>,
    target_to_pos: IndexSet<Vec<u8>>,

    // What values to fill in the records
    fill_query_id: bool,
    fill_query_name: bool,
    fill_target_ids: bool,
    fill_target_names: bool,
}

impl<'a, R: Read> Parser<'a, R> {

    pub fn new<T: Iterator<Item=Vec<u8>>, Q: Iterator<Item=Vec<u8>>>(
        conn_pseudoalns: &'a mut R,
        conn_query_names: Option<&mut Q>,
        targets: Option<&mut T>,
    ) -> Result<Self, E> {
        // Guess the input format
        let mut reader = BufReader::new(conn_pseudoalns);
        let mut buf = Cursor::new(Vec::<u8>::new());
        reader.read_until(b'\n', buf.get_mut())?;
        let format = guess_format(buf.get_ref())?;

        let mut ret = Self {
            reader, buf, format,
            query_to_pos: IndexSet::new(),
            target_to_pos: IndexSet::new(),
            fill_query_id: true,
            fill_query_name: true,
            fill_target_ids: true,
            fill_target_names: true,
        };

        if ret.format != Format::Metagraph && conn_query_names.is_none() {
            return Err(Box::new(crate::errors::NeedQueryNamesErr{ format: ret.format }))
        }

        let targets_from_header = ret.read_header()?;
        if let Some(targets) = targets {
            ret.target_to_pos = IndexSet::<Vec<u8>>::from_iter(targets);
        } else if let Some(targets) = targets_from_header {
            ret.target_to_pos = IndexSet::<Vec<u8>>::from_iter(targets);
        } else {
            return Err(Box::new(crate::errors::NeedTargetSequencesErr{ format: ret.format }))
        }

        if let Some(conn_query_names) = conn_query_names {
            ret.query_to_pos = IndexSet::<Vec<u8>>::from_iter(conn_query_names);
        }

        Ok(ret)
    }
}

impl<R: Read> Parser<'_, R> {
    /// Consumes the header line and returns the target sequence names.
    ///
    /// The header line is only present in Bifrost and Metagraph output. For
    /// Themisto and Fulgor, this will return None.
    ///
    /// Returns None if the header has already been consumed by calling [next].
    /// This is checked by looking whether target_to_pos contains anything.
    pub fn read_header(
        &mut self,
    ) -> Result<Option<Vec<Vec<u8>>>, E> {
        if !self.target_to_pos.is_empty() || self.buf.get_ref().is_empty() {
            return Ok(None)
        }
        match self.format {
            Format::Themisto => Ok(None),
            Format::Fulgor => Ok(None),
            Format::Metagraph => Ok(None),
            Format::Bifrost => {
                let separator: char = '\t';
                let contents: String = self.buf.get_ref().iter().map(|x| *x as char).collect();
                let mut records = contents.split(separator);
                // Consume `query_name`
                records.next().ok_or(crate::errors::CorruptedInputErr{})?;
                let mut target_names: Vec<Vec<u8>> = Vec::new();
                for record in records {
                    target_names.push(record.as_bytes().to_vec());
                }
                let n_targets = target_names.len();
                target_names[n_targets - 1].pop();
                self.buf.get_mut().clear();

                Ok(Some(target_names))
            }
            Format::SAM => {
                let mut header_contents = Cursor::new(self.buf.get_mut().clone());
                let mut next_line: Cursor<Vec<u8>> = Cursor::new(Vec::new());
                loop {
                    self.reader.read_until(b'\n', next_line.get_mut())?;
                    if next_line.get_ref().is_empty() {
                        break;
                    }
                    if next_line.get_ref()[0] == b'@' {
                        header_contents.get_mut().append(next_line.get_mut());
                    } else {
                        self.buf = next_line.clone();
                        break;
                    }
                }
                let mut reader = noodles_sam::io::reader::Builder::default().build_from_reader(&mut header_contents)?;
                let header = reader.read_header()?;
                let target_names: Vec<Vec<u8>> = header.reference_sequences().iter().map(|x| x.0.to_vec()).collect();
                Ok(Some(target_names))
            },
        }
    }

    /// Returns the number of query records in the input fastX file
    pub fn len(
        &self,
    ) -> usize {
        self.query_to_pos.len()
    }

    pub fn is_empty(
        &self,
    ) -> bool {
        self.query_to_pos.is_empty()
    }

    pub fn get_targets(
        &self,
    ) -> Option<Vec<Vec<u8>>> {
        Some(self.target_to_pos.iter().cloned().collect())
    }

    #[allow(clippy::unnecessary_unwrap)]
    fn fill_record(
        &mut self,
        record: &mut PseudoAln,
    ) {
        if record.query_id.is_none() && self.fill_query_id {
            let key: Vec<u8> = record.query_name.as_ref().unwrap().to_vec();
            let query_index = self.query_to_pos.get_index_of(&key).unwrap();
            record.query_id = Some(query_index as u32);
        }

        if record.query_name.is_none() && self.fill_query_name {
            let query_name = self.query_to_pos.get_index(record.query_id.unwrap() as usize).unwrap();
            record.query_name = Some(query_name.to_vec());
        }

        if record.ones_names.is_none() && record.ones.is_some() && self.fill_target_names {
            let ones_names = record.ones.as_ref().unwrap().iter().map(|target_idx| {
                self.target_to_pos.get_index(*target_idx as usize).unwrap().clone()
            }).collect::<Vec<Vec<u8>>>();
            record.ones_names = Some(ones_names);
        }

        if record.ones_names.is_some() && record.ones.is_none() && self.fill_target_ids{
            let ones = record.ones_names.as_ref().unwrap().iter().map(|target_name| {
                self.target_to_pos.get_index_of(target_name).unwrap() as u32
            }).collect::<Vec<u32>>();
            record.ones = Some(ones);
        }
    }

    pub fn fill_query_id(
        &mut self,
        val: bool,
    ) {
        self.fill_query_id = val;
    }

    pub fn fill_query_name(
        &mut self,
        val: bool,
    ) {
        self.fill_query_name = val;
    }

    pub fn fill_target_ids(
        &mut self,
        val: bool,
    ) {
        self.fill_target_ids = val;
    }

    pub fn fill_target_names(
        &mut self,
        val: bool,
    ) {
        self.fill_target_names = val;
    }
}

impl<R: Read> Iterator for Parser<'_, R> {
    type Item = PseudoAln;

    fn next(
        &mut self,
    ) -> Option<PseudoAln> {
        if self.buf.get_ref().is_empty() {
            let ret = self.reader.read_until(b'\n', self.buf.get_mut());
            if ret.is_err() || self.buf.get_ref().is_empty() {
                return None
            }
            self.buf.rewind().unwrap();
        }
        self.buf.get_mut().pop();

        let mut record = match self.format {
            Format::Themisto => read_themisto(&mut self.buf).unwrap(),
            Format::Fulgor => read_fulgor(&mut self.buf).unwrap(),
            Format::Metagraph => read_metagraph(&mut self.buf).unwrap(),
            Format::Bifrost => read_bifrost(&mut self.buf).unwrap(),
            Format::SAM => read_sam(&mut self.buf).unwrap(),
        };

        self.buf.get_mut().clear();

        self.fill_record(&mut record);
        Some(record)
    }
}

/// Guess the input format from plaintext bytes
///
/// Supports:
/// - SAM
/// - Themisto
/// - Bifrost
/// - Fulgor
/// - Metagraph
///
/// ## Errors
/// ### [crate::errors::CorruptedInputErr]
/// Input bytes do not contain the expected data.
///
/// ### [AmbiguousInputFormatErr]
/// Input format is either fulgor or metagraph but cannot be inferred with certainty.
///
/// ### [UnrecognizedInputFormatErr]
/// Could not infer input format.
///
pub fn guess_format(
    bytes: &[u8],
) -> Result<Format, E> {
    let first_line: Vec<u8> = if bytes.contains(&b'\n') {
        let linebreak = bytes.iter().position(|x| *x == b'\n').unwrap();
        bytes[0..linebreak].to_vec()
    } else {
        bytes.to_vec()
    };

    if bytes.len() > 2 {
        let sam: bool = bytes[0] == b'@' && bytes[1] == b'H' && bytes[2] == b'D';
        if sam {
            return Ok(Format::SAM)
        }
    }

    let not_themisto: bool = first_line.contains(&b'\t');
    if !not_themisto {
        return Ok(Format::Themisto)
    }

    let line = first_line.clone().iter().map(|x| *x as char).collect::<String>();
    let mut records = line.split('\t');

    let first_record = records.next().ok_or(crate::errors::CorruptedInputErr{})?;
    let bifrost: bool = first_record == "query_name";
    if bifrost {
        return Ok(Format::Bifrost)
    }

    let maybe_metagraph: bool = first_record.parse::<u32>().is_ok();

    let next = records.next().ok_or(crate::errors::CorruptedInputErr{})?;

    let fulgor: bool = next.parse::<u32>().is_ok();

    if fulgor && maybe_metagraph {
        return Err(Box::new(crate::errors::AmbiguousInputFormatErr{}))
    }

    if fulgor {
        return Ok(Format::Fulgor)
    }

    if maybe_metagraph {
        return Ok(Format::Metagraph)
    }

    Err(Box::new(crate::errors::UnrecognizedInputFormatErr{}))
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
    fn guess_format_metagraph() {
        use crate::Format;
        use super::guess_format;

        let mut data: Vec<u8> = b"30\tERR4035126.16\t\n".to_vec();
        data.append(&mut b"15084\tERR4035126.7543\tplasmid.fasta\n".to_vec());

        let got = guess_format(&data).unwrap();
        let expected = Format::Metagraph;

        assert_eq!(got, expected);
    }


    #[test]
    fn guess_format_sam() {
        use crate::Format;
        use super::guess_format;

        let mut data: Vec<u8> = b"@HD\tVN:1.5\tSO:unsorted\tGO:query\n".to_vec();
        data.append(&mut b"@SQ\tSN:OZ038621.1\tLN:5535987\n".to_vec());
        data.append(&mut b"@SQ\tSN:OZ038622.1\tLN:104814\n".to_vec());
        data.append(&mut b"@PG\tID:bwa\tPN:bwa\tVN:0.7.19-r1273\tCL:bwa mem -t 10 -o fwd_test.sam GCA_964037205.1_30348_1_60_genomic.fna ERR4035126_1.fastq.gz\n".to_vec());
        data.append(&mut b"ERR4035126.1\t16\tOZ038621.1\t4541508\t60\t151M\t*\t0\t0\tAGTATTTAGTGACCTAAGTCAATAAAATTTTAATTTACTCACGGCAGGTAACCAGTTCAGAAGCTGCTATCAGACACTCTTTTTTTAATCCACACAGAGACATATTGCCCGTTGCAGTCAGAATGAAAAGCTGAAAATCACTTACTAAGGC FJ<<JJFJAA<-JFAJFAF<JFFJJJJJJJFJFJJA<A<AJJAAAFFJJJJFJJFJFJAJJ7JJJJJFJJJJJFFJFFJFJJJJJJFJ7FFJAJJJJJJJJFJJFJJFJFJJJJFJJFJJJJJJJJJFFJJJJJJJJJJJJJFJJJFFAAA\tNM:i:0\tMD:Z:151\tAS:i:151\tXS:i:0\n".to_vec());

        let got = guess_format(&data).unwrap();
        let expected = Format::SAM;

        assert_eq!(got, expected);
    }

    #[test]
    fn consume_bifrost_header_with_next() {
        use super::Parser;
        use crate::PseudoAln;
        use std::io::Cursor;

        let mut data: Vec<u8> = b"query_name\tchr.fasta\tplasmid.fasta\n".to_vec();
        data.append(&mut b"ERR4035126.1\t121\t0\n".to_vec());
        let expected: PseudoAln = PseudoAln{ones_names: Some(vec!["chr.fasta".as_bytes().to_vec()]),  query_id: Some(0), ones: Some(vec![0]), query_name: Some("ERR4035126.1".as_bytes().to_vec()) };

        let mut cursor = Cursor::new(data);

        let targets = vec!["chr.fasta".as_bytes().to_vec(), "plasmid.fasta".as_bytes().to_vec()];
        let queries = vec!["ERR4035126.1".as_bytes().to_vec()];
        let mut it = queries.into_iter();
        let mut t_it = targets.into_iter();
        let mut reader = Parser::new(&mut cursor, Some(&mut it), Some(&mut t_it)).unwrap();

        let got: PseudoAln = reader.next().unwrap();

        assert_eq!(got, expected);
    }

    #[test]
    fn read_bifrost_header() {
        use super::Parser;
        use std::io::Cursor;

        let mut data: Vec<u8> = b"query_name\tchr.fasta\tplasmid.fasta\n".to_vec();
        data.append(&mut b"ERR4035126.1\t121\t0\n".to_vec());
        let expected: Vec<Vec<u8>> = vec!["chr.fasta".as_bytes().to_vec(), "plasmid.fasta".as_bytes().to_vec()];

        let mut cursor = Cursor::new(data);

        let targets = vec!["chr.fasta".as_bytes().to_vec(), "plasmid.fasta".as_bytes().to_vec()];
        let queries = vec!["ERR4035126.1".as_bytes().to_vec()];
        let mut it = queries.into_iter();
        let mut t_it = targets.into_iter();
        let reader = Parser::new(&mut cursor, Some(&mut it), Some(&mut t_it)).unwrap();

        let got = reader.get_targets().unwrap();

        assert_eq!(got, expected);
    }

    #[test]
    fn read_sam_header() {
        use super::Parser;
        use std::io::Cursor;

        let mut data: Vec<u8> = b"@HD\tVN:1.5\tSO:unsorted\tGO:query\n".to_vec();
        data.append(&mut b"@SQ\tSN:OZ038621.1\tLN:5535987\n".to_vec());
        data.append(&mut b"@SQ\tSN:OZ038622.1\tLN:104814\n".to_vec());
        data.append(&mut b"@PG\tID:bwa\tPN:bwa\tVN:0.7.19-r1273\tCL:bwa mem -t 10 -o fwd_test.sam GCA_964037205.1_30348_1_60_genomic.fna ERR4035126_1.fastq.gz\n".to_vec());
        data.append(&mut b"ERR4035126.1\t16\tOZ038621.1\t4541508\t60\t151M\t*\t0\t0\tAGTATTTAGTGACCTAAGTCAATAAAATTTTAATTTACTCACGGCAGGTAACCAGTTCAGAAGCTGCTATCAGACACTCTTTTTTTAATCCACACAGAGACATATTGCCCGTTGCAGTCAGAATGAAAAGCTGAAAATCACTTACTAAGGC FJ<<JJFJAA<-JFAJFAF<JFFJJJJJJJFJFJJA<A<AJJAAAFFJJJJFJJFJFJAJJ7JJJJJFJJJJJFFJFFJFJJJJJJFJ7FFJAJJJJJJJJFJJFJJFJFJJJJFJJFJJJJJJJJJFFJJJJJJJJJJJJJFJJJFFAAA\tNM:i:0\tMD:Z:151\tAS:i:151\tXS:i:0\n".to_vec());

        let expected: Vec<Vec<u8>> = vec!["OZ038621.1".as_bytes().to_vec(), "OZ038622.1".as_bytes().to_vec()];

        let mut cursor = Cursor::new(data);

        let targets = vec!["OZ038621.1".as_bytes().to_vec(), "OZ038622.1".as_bytes().to_vec()];
        let queries = vec!["ERR4035126.1".as_bytes().to_vec()];
        let mut it = queries.into_iter();
        let mut t_it = targets.into_iter();
        let reader = Parser::new(&mut cursor, Some(&mut it), Some(&mut t_it)).unwrap();

        let got = reader.get_targets().unwrap();

        assert_eq!(got, expected);
    }

    #[test]
    fn consume_sam_header_with_next() {
        use super::Parser;
        use crate::PseudoAln;
        use std::io::Cursor;

        let mut data: Vec<u8> = b"@HD\tVN:1.5\tSO:unsorted\tGO:query\n".to_vec();
        data.append(&mut b"@SQ\tSN:OZ038621.1\tLN:5535987\n".to_vec());
        data.append(&mut b"@SQ\tSN:OZ038622.1\tLN:104814\n".to_vec());
        data.append(&mut b"@PG\tID:bwa\tPN:bwa\tVN:0.7.19-r1273\tCL:bwa mem -t 10 -o fwd_test.sam GCA_964037205.1_30348_1_60_genomic.fna ERR4035126_1.fastq.gz\n".to_vec());
        data.append(&mut b"ERR4035126.1\t16\tOZ038621.1\t4541508\t60\t151M\t*\t0\t0\tAGTATTTAGTGACCTAAGTCAATAAAATTTTAATTTACTCACGGCAGGTAACCAGTTCAGAAGCTGCTATCAGACACTCTTTTTTTAATCCACACAGAGACATATTGCCCGTTGCAGTCAGAATGAAAAGCTGAAAATCACTTACTAAGGC FJ<<JJFJAA<-JFAJFAF<JFFJJJJJJJFJFJJA<A<AJJAAAFFJJJJFJJFJFJAJJ7JJJJJFJJJJJFFJFFJFJJJJJJFJ7FFJAJJJJJJJJFJJFJJFJFJJJJFJJFJJJJJJJJJFFJJJJJJJJJJJJJFJJJFFAAA\tNM:i:0\tMD:Z:151\tAS:i:151\tXS:i:0\n".to_vec());

        let expected = PseudoAln{ones_names: Some(vec!["OZ038621.1".as_bytes().to_vec()]), query_id: Some(0), ones: Some(vec![0]), query_name: Some("ERR4035126.1".as_bytes().to_vec()) };

        let mut cursor = Cursor::new(data);

        let targets = vec!["OZ038621.1".as_bytes().to_vec(), "OZ038622.1".as_bytes().to_vec()];
        let queries = vec!["ERR4035126.1".as_bytes().to_vec()];
        let mut it = queries.into_iter();
        let mut t_it = targets.into_iter();
        let mut reader = Parser::new(&mut cursor, Some(&mut it), Some(&mut t_it)).unwrap();

        let got: PseudoAln = reader.next().unwrap();

        assert_eq!(got, expected);
    }

    #[test]
    fn read_sam_header_and_first_line() {
        use super::Parser;
        use crate::PseudoAln;
        use std::io::Cursor;

        let mut data: Vec<u8> = b"@HD\tVN:1.5\tSO:unsorted\tGO:query\n".to_vec();
        data.append(&mut b"@SQ\tSN:OZ038621.1\tLN:5535987\n".to_vec());
        data.append(&mut b"@SQ\tSN:OZ038622.1\tLN:104814\n".to_vec());
        data.append(&mut b"@PG\tID:bwa\tPN:bwa\tVN:0.7.19-r1273\tCL:bwa mem -t 10 -o fwd_test.sam GCA_964037205.1_30348_1_60_genomic.fna ERR4035126_1.fastq.gz\n".to_vec());
        data.append(&mut b"ERR4035126.1\t16\tOZ038621.1\t4541508\t60\t151M\t*\t0\t0\tAGTATTTAGTGACCTAAGTCAATAAAATTTTAATTTACTCACGGCAGGTAACCAGTTCAGAAGCTGCTATCAGACACTCTTTTTTTAATCCACACAGAGACATATTGCCCGTTGCAGTCAGAATGAAAAGCTGAAAATCACTTACTAAGGC FJ<<JJFJAA<-JFAJFAF<JFFJJJJJJJFJFJJA<A<AJJAAAFFJJJJFJJFJFJAJJ7JJJJJFJJJJJFFJFFJFJJJJJJFJ7FFJAJJJJJJJJFJJFJJFJFJJJJFJJFJJJJJJJJJFFJJJJJJJJJJJJJFJJJFFAAA\tNM:i:0\tMD:Z:151\tAS:i:151\tXS:i:0\n".to_vec());

        let expected_header: Vec<Vec<u8>> = vec!["OZ038621.1".as_bytes().to_vec(), "OZ038622.1".as_bytes().to_vec()];
        let expected_aln = PseudoAln{ones_names: Some(vec!["OZ038621.1".as_bytes().to_vec()]), query_id: Some(0), ones: Some(vec![0]), query_name: Some("ERR4035126.1".as_bytes().to_vec()) };

        let mut cursor = Cursor::new(data);

        let targets = vec!["OZ038621.1".as_bytes().to_vec(), "OZ038622.1".as_bytes().to_vec()];
        let queries = vec!["ERR4035126.1".as_bytes().to_vec()];
        let mut it = queries.into_iter();
        let mut t_it = targets.into_iter();
        let mut reader = Parser::new(&mut cursor, Some(&mut it), Some(&mut t_it)).unwrap();

        let got_header = reader.get_targets().unwrap();
        assert_eq!(got_header, expected_header);

        let got_aln: PseudoAln = reader.next().unwrap();
        assert_eq!(got_aln, expected_aln);
    }

    #[test]
    fn read_sam_multiple() {
        use super::Parser;
        use crate::PseudoAln;
        use std::io::Cursor;

        let mut data: Vec<u8> = b"@HD\tVN:1.5\tSO:unsorted\tGO:query\n".to_vec();
        data.append(&mut b"@SQ\tSN:OZ038621.1\tLN:5535987\n".to_vec());
        data.append(&mut b"@SQ\tSN:OZ038622.1\tLN:104814\n".to_vec());
        data.append(&mut b"@PG\tID:bwa\tPN:bwa\tVN:0.7.19-r1273\tCL:bwa mem -t 10 -o fwd_test.sam GCA_964037205.1_30348_1_60_genomic.fna ERR4035126_1.fastq.gz\n".to_vec());
        data.append(&mut b"ERR4035126.1\t16\tOZ038621.1\t4541508\t60\t151M\t*\t0\t0\tAGTATTTAGTGACCTAAGTCAATAAAATTTTAATTTACTCACGGCAGGTAACCAGTTCAGAAGCTGCTATCAGACACTCTTTTTTTAATCCACACAGAGACATATTGCCCGTTGCAGTCAGAATGAAAAGCTGAAAATCACTTACTAAGGC FJ<<JJFJAA<-JFAJFAF<JFFJJJJJJJFJFJJA<A<AJJAAAFFJJJJFJJFJFJAJJ7JJJJJFJJJJJFFJFFJFJJJJJJFJ7FFJAJJJJJJJJFJJFJJFJFJJJJFJJFJJJJJJJJJFFJJJJJJJJJJJJJFJJJFFAAA\tNM:i:0\tMD:Z:151\tAS:i:151\tXS:i:0\n".to_vec());
        data.append(&mut b"ERR4035126.2\t16\tOZ038621.1\t4541557\t60\t151M\t*\t0\t0\tAACCAGTTCAGAAGCTGCTATCAGACACTCTTTTTTTAATCCACACAGAGACATATTGCCCGTTGCAGTCAGAATGAAAAGCTGAAAATCACTTACTAAGGCGTTTTTTATTTGGTGATATTTTTTTCAATATCATGCAGCAAACGGTGCA JAFJFJJJFFJFAJJJJJJJJJJFFA<JJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJFJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJFJJJJJJJJJJJJJJJFF-FFFAA\tNM:i:0\tMD:Z:151\tAS:i:151\tXS:i:0\n".to_vec());
        data.append(&mut b"ERR4035126.3\t16\tOZ038622.1\t4541521\t60\t151M\t*\t0\t0\tCTAAGTCAATAAAATTTTAATTTACTCACGGCAGGTAACCAGTTCAGAAGCTGCTATCAGACACTCTTTTTTTAATCCACACAGAGACATATTGCCCGTTGCAGTCAGAATGAAAAGCTGAAAATCACTTACTAAGGCGTTTTTTATTTGG JJJJJJJFJFFFJJJJJJAJJJF7JJJJJ<JJFFJJJJJJJFJJJJJJJJJFFFJJJFJJJJJJJJJJJJJJJJAJFJJJJFJJJJJJJJJJJJJJJJJJJJJJAJJJJJJJJJJJJJJJJJAJFJFJJJJJJJJJJJJJJJJJFJFAFAA\tNM:i:0\tMD:Z:151\tAS:i:151\tXS:i:0\n".to_vec());

        let expected = vec![
            PseudoAln{ones_names: Some(vec!["OZ038621.1".as_bytes().to_vec()]), query_id: Some(0), ones: Some(vec![0]), query_name: Some("ERR4035126.1".as_bytes().to_vec()) },
            PseudoAln{ones_names: Some(vec!["OZ038621.1".as_bytes().to_vec()]), query_id: Some(1), ones: Some(vec![0]), query_name: Some("ERR4035126.2".as_bytes().to_vec()) },
            PseudoAln{ones_names: Some(vec!["OZ038622.1".as_bytes().to_vec()]), query_id: Some(2), ones: Some(vec![1]), query_name: Some("ERR4035126.3".as_bytes().to_vec()) },
        ];

        let mut cursor = Cursor::new(data);

        let targets = vec!["OZ038621.1".as_bytes().to_vec(), "OZ038622.1".as_bytes().to_vec()];
        let queries = vec!["ERR4035126.1".as_bytes().to_vec(), "ERR4035126.2".as_bytes().to_vec(), "ERR4035126.3".as_bytes().to_vec()];
        let mut it = queries.into_iter();
        let mut t_it = targets.into_iter();
        let mut reader = Parser::new(&mut cursor, Some(&mut it), Some(&mut t_it)).unwrap();

        let mut got: Vec<PseudoAln> = Vec::new();
        while let Some(record) = reader.next() {
            got.push(record);
        }

        assert_eq!(got, expected);
    }

    #[test]
    fn parse_themisto_output() {
        use super::Parser;

        use crate::Format;
        use crate::PseudoAln;

        use std::io::Cursor;

        let data: Vec<u8> = vec![b"128 0 7 11 3\n".to_vec(),
                                 b"7 3 2 1 0\n".to_vec(),
                                 b"8\n".to_vec(),
                                 b"0\n".to_vec(),
                                 b"1 4 2 9 7\n".to_vec(),
        ].concat();

        let expected = vec![
            PseudoAln{ones_names: Some(vec!["0".as_bytes().to_vec(), "7".as_bytes().to_vec(), "11".as_bytes().to_vec(), "3".as_bytes().to_vec()]),  query_id: Some(128), ones: Some(vec![0, 7, 11, 3]), query_name: Some("128".as_bytes().to_vec())},
            PseudoAln{ones_names: Some(vec!["3".as_bytes().to_vec(), "2".as_bytes().to_vec(), "1".as_bytes().to_vec(), "0".as_bytes().to_vec()]),  query_id: Some(7),   ones: Some(vec![3, 2, 1, 0]), query_name: Some("7".as_bytes().to_vec())},
            PseudoAln{ones_names: Some(vec![]),  query_id: Some(8),   ones: Some(vec![]), query_name: Some("8".as_bytes().to_vec())},
            PseudoAln{ones_names: Some(vec![]),  query_id: Some(0),   ones: Some(vec![]), query_name: Some("0".as_bytes().to_vec())},
            PseudoAln{ones_names: Some(vec!["4".as_bytes().to_vec(), "2".as_bytes().to_vec(), "9".as_bytes().to_vec(), "7".as_bytes().to_vec()]),  query_id: Some(1),   ones: Some(vec![4, 2, 9, 7]), query_name: Some("1".as_bytes().to_vec())},
        ];

        let mut cursor: Cursor<Vec<u8>> = Cursor::new(data);

        let targets = vec![
            "0".as_bytes().to_vec(),
            "1".as_bytes().to_vec(),
            "2".as_bytes().to_vec(),
            "3".as_bytes().to_vec(),
            "4".as_bytes().to_vec(),
            "5".as_bytes().to_vec(),
            "6".as_bytes().to_vec(),
            "7".as_bytes().to_vec(),
            "8".as_bytes().to_vec(),
            "9".as_bytes().to_vec(),
            "10".as_bytes().to_vec(),
            "11".as_bytes().to_vec(),
        ];
        let queries = (0..129).map(|x| x.to_string().as_bytes().to_vec()).collect::<Vec<Vec<u8>>>();
        let mut it = queries.into_iter();
        let mut t_it = targets.into_iter();
        let mut reader = Parser::new(&mut cursor, Some(&mut it), Some(&mut t_it)).unwrap();

        let mut res: Vec<PseudoAln> = Vec::new();
        for record in reader.by_ref() {
            res.push(record);
        }

        let (got, got_format) = (res, reader.format);

        assert_eq!(got_format, Format::Themisto);
        assert_eq!(got, expected);
    }

    #[test]
    fn parse_fulgor_output() {
        use super::Parser;

        use crate::Format;
        use crate::PseudoAln;

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
            PseudoAln{ones_names: Some(vec![]),  query_id: Some(0), ones: Some(vec![]), query_name: Some("ERR4035126.4996".as_bytes().to_vec()) },
            PseudoAln{ones_names: Some(vec!["chr.fasta".as_bytes().to_vec()]),  query_id: Some(1), ones: Some(vec![0]), query_name: Some("ERR4035126.1262953".as_bytes().to_vec()) },
            PseudoAln{ones_names: Some(vec!["plasmid.fasta".as_bytes().to_vec()]),  query_id: Some(2), ones: Some(vec![1]), query_name: Some("ERR4035126.1262954".as_bytes().to_vec()) },
            PseudoAln{ones_names: Some(vec!["plasmid.fasta".as_bytes().to_vec()]),  query_id: Some(3), ones: Some(vec![1]), query_name: Some("ERR4035126.1262955".as_bytes().to_vec()) },
            PseudoAln{ones_names: Some(vec!["chr.fasta".as_bytes().to_vec()]),  query_id: Some(4), ones: Some(vec![0]), query_name: Some("ERR4035126.1262956".as_bytes().to_vec()) },
            PseudoAln{ones_names: Some(vec!["chr.fasta".as_bytes().to_vec()]),  query_id: Some(5), ones: Some(vec![0]), query_name: Some("ERR4035126.1262957".as_bytes().to_vec()) },
            PseudoAln{ones_names: Some(vec!["chr.fasta".as_bytes().to_vec()]),  query_id: Some(6), ones: Some(vec![0]), query_name: Some("ERR4035126.1262958".as_bytes().to_vec()) },
            PseudoAln{ones_names: Some(vec!["chr.fasta".as_bytes().to_vec()]),  query_id: Some(7), ones: Some(vec![0]), query_name: Some("ERR4035126.1262959".as_bytes().to_vec()) },
            PseudoAln{ones_names: Some(vec!["chr.fasta".as_bytes().to_vec(), "plasmid.fasta".as_bytes().to_vec()]),  query_id: Some(8), ones: Some(vec![0, 1]), query_name: Some("ERR4035126.651965".as_bytes().to_vec()) },
            PseudoAln{ones_names: Some(vec![]),  query_id: Some(9), ones: Some(vec![]), query_name: Some("ERR4035126.11302".as_bytes().to_vec()) },
            PseudoAln{ones_names: Some(vec!["chr.fasta".as_bytes().to_vec()]),  query_id: Some(10), ones: Some(vec![0]), query_name: Some("ERR4035126.1262960".as_bytes().to_vec()) },
            PseudoAln{ones_names: Some(vec!["chr.fasta".as_bytes().to_vec()]),  query_id: Some(11), ones: Some(vec![0]), query_name: Some("ERR4035126.1262961".as_bytes().to_vec()) },
            PseudoAln{ones_names: Some(vec!["chr.fasta".as_bytes().to_vec()]),  query_id: Some(12), ones: Some(vec![0]), query_name: Some("ERR4035126.1262962".as_bytes().to_vec()) },
            PseudoAln{ones_names: Some(vec!["chr.fasta".as_bytes().to_vec(), "plasmid.fasta".as_bytes().to_vec()]),  query_id: Some(8), ones: Some(vec![0, 1]), query_name: Some("ERR4035126.651965".as_bytes().to_vec()) },
        ];

        let mut cursor: Cursor<Vec<u8>> = Cursor::new(data);

        let targets = vec!["chr.fasta".as_bytes().to_vec(), "plasmid.fasta".as_bytes().to_vec()];
        let queries = vec![
            "ERR4035126.4996".as_bytes().to_vec(),
            "ERR4035126.1262953".as_bytes().to_vec(),
            "ERR4035126.1262954".as_bytes().to_vec(),
            "ERR4035126.1262955".as_bytes().to_vec(),
            "ERR4035126.1262956".as_bytes().to_vec(),
            "ERR4035126.1262957".as_bytes().to_vec(),
            "ERR4035126.1262958".as_bytes().to_vec(),
            "ERR4035126.1262959".as_bytes().to_vec(),
            "ERR4035126.651965".as_bytes().to_vec(),
            "ERR4035126.11302".as_bytes().to_vec(),
            "ERR4035126.1262960".as_bytes().to_vec(),
            "ERR4035126.1262961".as_bytes().to_vec(),
            "ERR4035126.1262962".as_bytes().to_vec(),
        ];
        let mut it = queries.into_iter();
        let mut t_it = targets.into_iter();
        let mut reader = Parser::new(&mut cursor, Some(&mut it), Some(&mut t_it)).unwrap();

        let mut res: Vec<PseudoAln> = Vec::new();
        for record in reader.by_ref() {
            res.push(record);
        }

        let (got, got_format) = (res, reader.format);

        assert_eq!(got_format, Format::Fulgor);
        assert_eq!(got, expected);
    }

    #[test]
    fn parse_bifrost_output() {
        use super::Parser;

        use crate::Format;
        use crate::PseudoAln;

        use std::io::Cursor;

        let mut data: Vec<u8> = b"query_name\tchromosome.fasta\tplasmid.fasta\n".to_vec();
        data.append(&mut b"ERR4035126.724962\t0\t0\n".to_vec());
        data.append(&mut b"ERR4035126.1235744\t0\t0\n".to_vec());
        data.append(&mut b"ERR4035126.431001\t0\t0\n".to_vec());
        data.append(&mut b"ERR4035126.645400\t0\t0\n".to_vec());
        data.append(&mut b"ERR4035126.3001\t121\t0\n".to_vec());
        data.append(&mut b"ERR4035126.515778\t242\t0\n".to_vec());
        data.append(&mut b"ERR4035126.886205\t121\t0\n".to_vec());
        data.append(&mut b"ERR4035126.1254676\t121\t0\n".to_vec());
        data.append(&mut b"ERR4035126.668031\t0\t121\n".to_vec());
        data.append(&mut b"ERR4035126.388619\t121\t0\n".to_vec());
        data.append(&mut b"ERR4035126.959743\t0\t0\n".to_vec());
        data.append(&mut b"ERR4035126.1146685\t0\t0\n".to_vec());
        data.append(&mut b"ERR4035126.1017809\t0\t0\n".to_vec());
        data.append(&mut b"ERR4035126.788136\t0\t0\n".to_vec());
        data.append(&mut b"ERR4035126.1223924\t366\t9\n".to_vec());
        data.append(&mut b"ERR4035126.910807\t0\t0\n".to_vec());
        data.append(&mut b"ERR4035126.824748\t80\t0\n".to_vec());

        let expected = vec![
            PseudoAln{ query_name: Some("ERR4035126.724962".as_bytes().to_vec()), ones: Some(vec![]), ones_names: Some(vec![]), query_id: Some(0) },
            PseudoAln{ query_name: Some("ERR4035126.1235744".as_bytes().to_vec()), ones: Some(vec![]), ones_names: Some(vec![]), query_id: Some(1) },
            PseudoAln{ query_name: Some("ERR4035126.431001".as_bytes().to_vec()), ones: Some(vec![]), ones_names: Some(vec![]), query_id: Some(2) },
            PseudoAln{ query_name: Some("ERR4035126.645400".as_bytes().to_vec()), ones: Some(vec![]), ones_names: Some(vec![]), query_id: Some(3) },
            PseudoAln{ query_name: Some("ERR4035126.3001".as_bytes().to_vec()), ones: Some(vec![0]), ones_names: Some(vec!["chromosome.fasta".as_bytes().to_vec()]), query_id: Some(4) },
            PseudoAln{ query_name: Some("ERR4035126.515778".as_bytes().to_vec()), ones: Some(vec![0]), ones_names: Some(vec!["chromosome.fasta".as_bytes().to_vec()]), query_id: Some(5) },
            PseudoAln{ query_name: Some("ERR4035126.886205".as_bytes().to_vec()), ones: Some(vec![0]), ones_names: Some(vec!["chromosome.fasta".as_bytes().to_vec()]), query_id: Some(6) },
            PseudoAln{ query_name: Some("ERR4035126.1254676".as_bytes().to_vec()), ones: Some(vec![0]), ones_names: Some(vec!["chromosome.fasta".as_bytes().to_vec()]), query_id: Some(7) },
            PseudoAln{ query_name: Some("ERR4035126.668031".as_bytes().to_vec()), ones: Some(vec![1]), ones_names: Some(vec!["plasmid.fasta".as_bytes().to_vec()]), query_id: Some(8) },
            PseudoAln{ query_name: Some("ERR4035126.388619".as_bytes().to_vec()), ones: Some(vec![0]), ones_names: Some(vec!["chromosome.fasta".as_bytes().to_vec()]), query_id: Some(9) },
            PseudoAln{ query_name: Some("ERR4035126.959743".as_bytes().to_vec()), ones: Some(vec![]), ones_names: Some(vec![]), query_id: Some(10) },
            PseudoAln{ query_name: Some("ERR4035126.1146685".as_bytes().to_vec()), ones: Some(vec![]), ones_names: Some(vec![]), query_id: Some(11) },
            PseudoAln{ query_name: Some("ERR4035126.1017809".as_bytes().to_vec()), ones: Some(vec![]), ones_names: Some(vec![]), query_id: Some(12) },
            PseudoAln{ query_name: Some("ERR4035126.788136".as_bytes().to_vec()), ones: Some(vec![]), ones_names: Some(vec![]), query_id: Some(13) },
            PseudoAln{ query_name: Some("ERR4035126.1223924".as_bytes().to_vec()), ones: Some(vec![0, 1]), ones_names: Some(vec!["chromosome.fasta".as_bytes().to_vec(), "plasmid.fasta".as_bytes().to_vec()]), query_id: Some(14) },
            PseudoAln{ query_name: Some("ERR4035126.910807".as_bytes().to_vec()), ones: Some(vec![]), ones_names: Some(vec![]), query_id: Some(15) },
            PseudoAln{ query_name: Some("ERR4035126.824748".as_bytes().to_vec()), ones: Some(vec![0]), ones_names: Some(vec!["chromosome.fasta".as_bytes().to_vec()]), query_id: Some(16) },
        ];

        let mut cursor: Cursor<Vec<u8>> = Cursor::new(data);

        let targets = vec!["chromosome.fasta".as_bytes().to_vec(), "plasmid.fasta".as_bytes().to_vec()];
        let queries = vec![
            "ERR4035126.724962".as_bytes().to_vec(),
            "ERR4035126.1235744".as_bytes().to_vec(),
            "ERR4035126.431001".as_bytes().to_vec(),
            "ERR4035126.645400".as_bytes().to_vec(),
            "ERR4035126.3001".as_bytes().to_vec(),
            "ERR4035126.515778".as_bytes().to_vec(),
            "ERR4035126.886205".as_bytes().to_vec(),
            "ERR4035126.1254676".as_bytes().to_vec(),
            "ERR4035126.668031".as_bytes().to_vec(),
            "ERR4035126.388619".as_bytes().to_vec(),
            "ERR4035126.959743".as_bytes().to_vec(),
            "ERR4035126.1146685".as_bytes().to_vec(),
            "ERR4035126.1017809".as_bytes().to_vec(),
            "ERR4035126.788136".as_bytes().to_vec(),
            "ERR4035126.1223924".as_bytes().to_vec(),
            "ERR4035126.910807".as_bytes().to_vec(),
            "ERR4035126.824748".as_bytes().to_vec(),
        ];
        let mut it = queries.into_iter();
        let mut t_it = targets.into_iter();
        let mut reader = Parser::new(&mut cursor, Some(&mut it), Some(&mut t_it)).unwrap();

        let mut res: Vec<PseudoAln> = Vec::new();
        for record in reader.by_ref() {
            res.push(record);
        }

        let (got, got_format) = (res, reader.format);

        assert_eq!(got_format, Format::Bifrost);
        assert_eq!(got, expected);
    }

    #[test]
    fn parse_bifrost_output_with_targets_from_data() {
        use super::Parser;

        use crate::Format;
        use crate::PseudoAln;

        use std::io::Cursor;

        let mut data: Vec<u8> = b"query_name\tchromosome.fasta\tplasmid.fasta\n".to_vec();
        data.append(&mut b"ERR4035126.724962\t0\t0\n".to_vec());
        data.append(&mut b"ERR4035126.1235744\t0\t0\n".to_vec());
        data.append(&mut b"ERR4035126.431001\t0\t0\n".to_vec());
        data.append(&mut b"ERR4035126.645400\t0\t0\n".to_vec());
        data.append(&mut b"ERR4035126.3001\t121\t0\n".to_vec());
        data.append(&mut b"ERR4035126.515778\t242\t0\n".to_vec());
        data.append(&mut b"ERR4035126.886205\t121\t0\n".to_vec());
        data.append(&mut b"ERR4035126.1254676\t121\t0\n".to_vec());
        data.append(&mut b"ERR4035126.668031\t0\t121\n".to_vec());
        data.append(&mut b"ERR4035126.388619\t121\t0\n".to_vec());
        data.append(&mut b"ERR4035126.959743\t0\t0\n".to_vec());
        data.append(&mut b"ERR4035126.1146685\t0\t0\n".to_vec());
        data.append(&mut b"ERR4035126.1017809\t0\t0\n".to_vec());
        data.append(&mut b"ERR4035126.788136\t0\t0\n".to_vec());
        data.append(&mut b"ERR4035126.1223924\t366\t9\n".to_vec());
        data.append(&mut b"ERR4035126.910807\t0\t0\n".to_vec());
        data.append(&mut b"ERR4035126.824748\t80\t0\n".to_vec());

        let expected = vec![
            PseudoAln{ query_name: Some("ERR4035126.724962".as_bytes().to_vec()), ones: Some(vec![]), ones_names: Some(vec![]), query_id: Some(0) },
            PseudoAln{ query_name: Some("ERR4035126.1235744".as_bytes().to_vec()), ones: Some(vec![]), ones_names: Some(vec![]), query_id: Some(1) },
            PseudoAln{ query_name: Some("ERR4035126.431001".as_bytes().to_vec()), ones: Some(vec![]), ones_names: Some(vec![]), query_id: Some(2) },
            PseudoAln{ query_name: Some("ERR4035126.645400".as_bytes().to_vec()), ones: Some(vec![]), ones_names: Some(vec![]), query_id: Some(3) },
            PseudoAln{ query_name: Some("ERR4035126.3001".as_bytes().to_vec()), ones: Some(vec![0]), ones_names: Some(vec!["chromosome.fasta".as_bytes().to_vec()]), query_id: Some(4) },
            PseudoAln{ query_name: Some("ERR4035126.515778".as_bytes().to_vec()), ones: Some(vec![0]), ones_names: Some(vec!["chromosome.fasta".as_bytes().to_vec()]), query_id: Some(5) },
            PseudoAln{ query_name: Some("ERR4035126.886205".as_bytes().to_vec()), ones: Some(vec![0]), ones_names: Some(vec!["chromosome.fasta".as_bytes().to_vec()]), query_id: Some(6) },
            PseudoAln{ query_name: Some("ERR4035126.1254676".as_bytes().to_vec()), ones: Some(vec![0]), ones_names: Some(vec!["chromosome.fasta".as_bytes().to_vec()]), query_id: Some(7) },
            PseudoAln{ query_name: Some("ERR4035126.668031".as_bytes().to_vec()), ones: Some(vec![1]), ones_names: Some(vec!["plasmid.fasta".as_bytes().to_vec()]), query_id: Some(8) },
            PseudoAln{ query_name: Some("ERR4035126.388619".as_bytes().to_vec()), ones: Some(vec![0]), ones_names: Some(vec!["chromosome.fasta".as_bytes().to_vec()]), query_id: Some(9) },
            PseudoAln{ query_name: Some("ERR4035126.959743".as_bytes().to_vec()), ones: Some(vec![]), ones_names: Some(vec![]), query_id: Some(10) },
            PseudoAln{ query_name: Some("ERR4035126.1146685".as_bytes().to_vec()), ones: Some(vec![]), ones_names: Some(vec![]), query_id: Some(11) },
            PseudoAln{ query_name: Some("ERR4035126.1017809".as_bytes().to_vec()), ones: Some(vec![]), ones_names: Some(vec![]), query_id: Some(12) },
            PseudoAln{ query_name: Some("ERR4035126.788136".as_bytes().to_vec()), ones: Some(vec![]), ones_names: Some(vec![]), query_id: Some(13) },
            PseudoAln{ query_name: Some("ERR4035126.1223924".as_bytes().to_vec()), ones: Some(vec![0, 1]), ones_names: Some(vec!["chromosome.fasta".as_bytes().to_vec(), "plasmid.fasta".as_bytes().to_vec()]), query_id: Some(14) },
            PseudoAln{ query_name: Some("ERR4035126.910807".as_bytes().to_vec()), ones: Some(vec![]), ones_names: Some(vec![]), query_id: Some(15) },
            PseudoAln{ query_name: Some("ERR4035126.824748".as_bytes().to_vec()), ones: Some(vec![0]), ones_names: Some(vec!["chromosome.fasta".as_bytes().to_vec()]), query_id: Some(16) },
        ];

        let mut cursor: Cursor<Vec<u8>> = Cursor::new(data);

        let queries = vec![
            "ERR4035126.724962".as_bytes().to_vec(),
            "ERR4035126.1235744".as_bytes().to_vec(),
            "ERR4035126.431001".as_bytes().to_vec(),
            "ERR4035126.645400".as_bytes().to_vec(),
            "ERR4035126.3001".as_bytes().to_vec(),
            "ERR4035126.515778".as_bytes().to_vec(),
            "ERR4035126.886205".as_bytes().to_vec(),
            "ERR4035126.1254676".as_bytes().to_vec(),
            "ERR4035126.668031".as_bytes().to_vec(),
            "ERR4035126.388619".as_bytes().to_vec(),
            "ERR4035126.959743".as_bytes().to_vec(),
            "ERR4035126.1146685".as_bytes().to_vec(),
            "ERR4035126.1017809".as_bytes().to_vec(),
            "ERR4035126.788136".as_bytes().to_vec(),
            "ERR4035126.1223924".as_bytes().to_vec(),
            "ERR4035126.910807".as_bytes().to_vec(),
            "ERR4035126.824748".as_bytes().to_vec(),
        ];
        let mut it = queries.into_iter();
        let mut reader = Parser::new(&mut cursor, Some(&mut it),  None::<&mut std::iter::Empty<Vec<u8>>>).unwrap();

        let mut res: Vec<PseudoAln> = Vec::new();
        for record in reader.by_ref() {
            res.push(record);
        }

        let (got, got_format) = (res, reader.format);

        assert_eq!(got_format, Format::Bifrost);
        assert_eq!(got, expected);
    }

    #[test]
    fn parse_metgraph_output() {
        use super::Parser;

        use crate::Format;
        use crate::PseudoAln;

        use std::io::Cursor;

        let mut data: Vec<u8> = b"3\tERR4035126.2\tchr.fasta\n".to_vec();
        data.append(&mut b"2\tERR4035126.1\tchr.fasta\n".to_vec());
        data.append(&mut b"1303804\tERR4035126.651903\tchr.fasta:plasmid.fasta\n".to_vec());
        data.append(&mut b"30\tERR4035126.16\t\n".to_vec());
        data.append(&mut b"15084\tERR4035126.7543\tplasmid.fasta\n".to_vec());

        let expected = vec![
            PseudoAln{ones_names: Some(vec!["chr.fasta".as_bytes().to_vec()]),  query_id: Some(3), ones: Some(vec![0]), query_name: Some("ERR4035126.2".as_bytes().to_vec()) },
            PseudoAln{ones_names: Some(vec!["chr.fasta".as_bytes().to_vec()]),  query_id: Some(2), ones: Some(vec![0]), query_name: Some("ERR4035126.1".as_bytes().to_vec()) },
            PseudoAln{ones_names: Some(vec!["chr.fasta".as_bytes().to_vec(), "plasmid.fasta".as_bytes().to_vec()]),  query_id: Some(1303804), ones: Some(vec![0, 1]), query_name: Some("ERR4035126.651903".as_bytes().to_vec()) },
            PseudoAln{ones_names: Some(vec![]),  query_id: Some(30), ones: Some(vec![]), query_name: Some("ERR4035126.16".as_bytes().to_vec()) },
            PseudoAln{ones_names: Some(vec!["plasmid.fasta".as_bytes().to_vec()]),  query_id: Some(15084), ones: Some(vec![1]), query_name: Some("ERR4035126.7543".as_bytes().to_vec()) },
        ];

        let mut cursor: Cursor<Vec<u8>> = Cursor::new(data);

        let targets = vec!["chr.fasta".as_bytes().to_vec(), "plasmid.fasta".as_bytes().to_vec()];
        let queries = vec![
            "ERR4035126.2".as_bytes().to_vec(),
            "ERR4035126.1".as_bytes().to_vec(),
            "ERR4035126.651903".as_bytes().to_vec(),
            "ERR4035126.16".as_bytes().to_vec(),
            "ERR4035126.7543".as_bytes().to_vec(),
        ];
        let mut it = queries.into_iter();
        let mut t_it = targets.into_iter();
        let mut reader = Parser::new(&mut cursor, Some(&mut it), Some(&mut t_it)).unwrap();

        let mut res: Vec<PseudoAln> = Vec::new();
        for record in reader.by_ref() {
            res.push(record);
        }

        let (got, got_format) = (res, reader.format);

        assert_eq!(got_format, Format::Metagraph);
        assert_eq!(got, expected);
    }

    #[test]
    fn parse_sam_output() {
        use super::Parser;

        use crate::Format;
        use crate::PseudoAln;

        use std::io::Cursor;

        let mut data: Vec<u8> = b"@HD\tVN:1.5\tSO:unsorted\tGO:query\n".to_vec();
        data.append(&mut b"@SQ\tSN:OZ038621.1\tLN:5535987\n".to_vec());
        data.append(&mut b"@SQ\tSN:OZ038622.1\tLN:104814\n".to_vec());
        data.append(&mut b"@PG\tID:bwa\tPN:bwa\tVN:0.7.19-r1273\tCL:bwa mem -t 10 -o fwd_test.sam GCA_964037205.1_30348_1_60_genomic.fna ERR4035126_1.fastq.gz\n".to_vec());
        data.append(&mut b"ERR4035126.1\t16\tOZ038621.1\t4541508\t60\t151M\t*\t0\t0\tAGTATTTAGTGACCTAAGTCAATAAAATTTTAATTTACTCACGGCAGGTAACCAGTTCAGAAGCTGCTATCAGACACTCTTTTTTTAATCCACACAGAGACATATTGCCCGTTGCAGTCAGAATGAAAAGCTGAAAATCACTTACTAAGGC\tFJ<<JJFJAA<-JFAJFAF<JFFJJJJJJJFJFJJA<A<AJJAAAFFJJJJFJJFJFJAJJ7JJJJJFJJJJJFFJFFJFJJJJJJFJ7FFJAJJJJJJJJFJJFJJFJFJJJJFJJFJJJJJJJJJFFJJJJJJJJJJJJJFJJJFFAAA\tNM:i:0\tMD:Z:151\tAS:i:151\tXS:i:0\n".to_vec());
        data.append(&mut b"ERR4035126.2\t16\tOZ038621.1\t4541557\t60\t151M\t*\t0\t0\tAACCAGTTCAGAAGCTGCTATCAGACACTCTTTTTTTAATCCACACAGAGACATATTGCCCGTTGCAGTCAGAATGAAAAGCTGAAAATCACTTACTAAGGCGTTTTTTATTTGGTGATATTTTTTTCAATATCATGCAGCAAACGGTGCA\tJAFJFJJJFFJFAJJJJJJJJJJFFA<JJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJFJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJFJJJJJJJJJJJJJJJFF-FFFAA\tNM:i:0\tMD:Z:151\tAS:i:151\tXS:i:0\n".to_vec());
        data.append(&mut b"ERR4035126.3\t16\tOZ038621.1\t4541521\t60\t151M\t*\t0\t0\tCTAAGTCAATAAAATTTTAATTTACTCACGGCAGGTAACCAGTTCAGAAGCTGCTATCAGACACTCTTTTTTTAATCCACACAGAGACATATTGCCCGTTGCAGTCAGAATGAAAAGCTGAAAATCACTTACTAAGGCGTTTTTTATTTGG\tJJJJJJJFJFFFJJJJJJAJJJF7JJJJJ<JJFFJJJJJJJFJJJJJJJJJFFFJJJFJJJJJJJJJJJJJJJJAJFJJJJFJJJJJJJJJJJJJJJJJJJJJJAJJJJJJJJJJJJJJJJJAJFJFJJJJJJJJJJJJJJJJJFJFAFAA\tNM:i:0\tMD:Z:151\tAS:i:151\tXS:i:0\n".to_vec());
        data.append(&mut b"ERR4035126.1261584\t16\tOZ038622.1\t66398\t60\t151M\t*\t0\t0\tGCCGCTGTCTGAACCATGATATTGGCGGAACCGATGCCCATGATGGATGCGCCCCACAGCATGACCAGTTGCGCCAGACTCCAGCCGGAAGCGGTGGGCACAATCATCAAAAATCCACTCACGACACTGAGTATGCCGACGACGTCCCGTC\tFFJJJJFFJFJFFFJJJJJJJJJJJ7FA<JJ<JFJJFJJJJF-FJJA<FJJJJAJJJJJJJJJJJJJJJJJJJJFFJJJFJJJJJJJJJJJJJJJJJJFJFJJJJJJJJJFJJJJJFJJFJFJFJJJJJJJJJJJJJJJJJJJJJJFFFAA\tNM:i:0\tMD:Z:151\tAS:i:151\tXS:i:0\n".to_vec());
        data.append(&mut b"ERR4035126.1213410\t16\tOZ038622.1\t3996\t60\t151M\t*\t0\t0\tGCTGGCGCTTCGGGGATATGTGTTTCGACGGCAGATGAATTTATTCCGGCGGGGGCTGATTCTGCCGTCTGTTCAGTAAATACAGGTGCGATAATATCTGTTTTTTCGGATAAGGACGGTGGCGAAAAAGTACGACGTTTTTTCACCACAA\tJJJJJJJJJJJJJJJJJJJJJJJJJJJFJFJFFJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJFJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJFFJJJJJJJJJJJFFAAA\tNM:i:0\tMD:Z:151\tAS:i:151\tXS:i:0\n".to_vec());
        data.append(&mut b"ERR4035126.1213410\t16\tOZ038621.1\t3996\t60\t151M\t*\t0\t0\tGCTGGCGCTTCGGGGATATGTGTTTCGACGGCAGATGAATTTATTCCGGCGGGGGCTGATTCTGCCGTCTGTTCAGTAAATACAGGTGCGATAATATCTGTTTTTTCGGATAAGGACGGTGGCGAAAAAGTACGACGTTTTTTCACCACAA\tJJJJJJJJJJJJJJJJJJJJJJJJJJJFJFJFFJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJFJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJFFJJJJJJJJJJJFFAAA\tNM:i:0\tMD:Z:151\tAS:i:151\tXS:i:0\n".to_vec());
        data.append(&mut b"ERR4035126.4\t0\tOZ038621.1\t4541351\t60\t151M\t*\t0\t0\tAGGTGCGGGCTTTTTTCTGTGTTTCCTGTACGCGTCAGCCCGCACCGTTACCTGTGGTAATGGTGATGGTGGTGGTAATGGTGGTGCTAATGCGTTTCATGGATGTTGTGTACTCTGTAATTTTTATCTGTCTGTGCGCTATGCCTATATT\tAAFFFJJJJJJJJJJJJJJJJJJJJJFFJJJJJJJJJJJJJJJJJJJFFJJJJJJJJFJJJJJJJJJJJ<JFJJJJJJJJJJJAJJJFJJJJJJJJJJJJJJJJJJJFJJJJJJJJJJJJJJJJJJJJJJJFFFJFAFJJJJF<FFFJJJJ\tNM:i:0\tMD:Z:151\tAS:i:151\tXS:i:0\n".to_vec());
        data.append(&mut b"ERR4035126.5\t16\tOZ038621.1\t4541533\t60\t151M\t*\t0\t0\tAATTTTAATTTACTCACGGCAGGTAACCAGTTCAGAAGCTGCTATCAGACACTCTTTTTTTAATCCACACAGAGACATATTGCCCGTTGCAGTCAGAATGAAAAGCTGAAAATCACTTACTAAGGCGTTTTTTATTTGGTGATATTTTTTT\tFJJJJJJJJFJJJJJJJJFJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJFJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJFJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJFJJJJJJJJJJJJJFFFAA\tNM:i:0\tMD:Z:151\tAS:i:151\tXS:i:0\n".to_vec());
        data.append(&mut b"ERR4035126.6\t0\tOZ038621.1\t4541261\t60\t151M\t*\t0\t0\tTCTGCATTTGCCACTGATGTACCGCCGAACTTCAACACTCGCATGGTTGTTACCTCGTTACCTTTGGTCGAAAAAAAAGCCCGCACTGTCAGGTGCGGGCTTTTTTCTGTGTTTCCTGTACGCGTCAGCCCGCACCGTTACCTGTGGTAAT\tAAAFFJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJFJJFJJJJJJJJ<FJJJJJJJJJJJJJJJJJJJJJJJJJJJFJJJJJJJJFJJJJF7FJJJJJJJFJFJJJJJJJJFJJJJJJJJAJJJJFJFFFJFJF\tNM:i:0\tMD:Z:151\tAS:i:151\tXS:i:0\n".to_vec());
        data.append(&mut b"ERR4035126.973529\t16\tOZ038621.1\t3695316\t60\t66S85M\t*\t0\t0\tGGAGATGATTTCGTGTTTCTTCTCCGGGATGACCATGTCATCGATACCAACAGATGCACCAGAACGCGCCAAGTCGGGCAATCTGGTGAACTGGAAAGCCGGGGCGCTGTATCACCTGACGGAAAACGGCAATGTCTATATTAACTATGCC\tJJFJFF7-FFJJJA-FJFFFJJFAJJJJJJJJJJJJJJJJFJJJJJJJJFJJJJFJ<JJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJFFJFJJJJJJJJFJJJJJJJJFAJJJJJJJJJJJJJFJJJJJFFJJJFJJJJJJFJJFFFAA\tNM:i:0\tMD:Z:85\tAS:i:85\tXS:i:0\tSA:Z:OZ038621.1,5194124,-,69M82S,60,0;\n".to_vec());
        data.append(&mut b"ERR4035126.973529\t2064\tOZ038621.1\t5194124\t60\t69M82H\t*\t0\t0\tGGAGATGATTTCGTGTTTCTTCTCCGGGATGACCATGTCATCGATACCAACAGATGCACCAGAACGCGC\tJJFJFF7-FFJJJA-FJFFFJJFAJJJJJJJJJJJJJJJJFJJJJJJJJFJJJJFJ<JJJJJJJJJJJJ\tNM:i:0\tMD:Z:69\tAS:i:69\tXS:i:0\tSA:Z:OZ038621.1,3695316,-,66S85M,60,0;\n".to_vec());
        data.append(&mut b"ERR4035126.621281\t16\tOZ038621.1\t1040569\t60\t39S86M26S\t*\t0\t0\tGCTCGACCGCGTCCCAGTTGAAATGCAACTCCCCAGCCAACTCGATAAACACGATGATTAACACGGCAGTCATGGTCAGAATGGAAACGGGATCGAAAATCGGCATACCAAATGACATCGGCGTGCCACAGCACAAACTGGACGCCCTGGC\tAFAJJJJJJJJJJJJFJJJJJJJJJJJJJJJJFJFJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJFFFAA\tNM:i:0\tMD:Z:86\tAS:i:86\tXS:i:0\tSA:Z:OZ038621.1,3172373,-,46M105S,60,0;OZ038621.1,1301509,+,33M118S,60,0;\n".to_vec());
        data.append(&mut b"ERR4035126.1178767\t4\t*\t0\t0\t*\t*\t0\t0\tACTTGGCTCATGTTCCGTCAATGCCGGAGAGACAATTGAAGTTGATTTAGGTGATGTCTTCGCTGCCAATTTCCGTGTTGTAGGGCATAAACCTCTTGGGGCCAGAACGGCAGAACTTGCAATTCCAGTCAGGTGTAACACGGGAAACGCG\tAAFFFJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJFJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJFJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJ\tAS:i:0\tXS:i:0\n".to_vec());
        data.append(&mut b"ERR4035126.621281\t2064\tOZ038621.1\t3172373\t60\t46M105H\t*\t0\t0\tGCTCGACCGCGTCCCAGTTGAAATGCAACTCCCCAGCCAACTCGAT\tAFAJJJJJJJJJJJJFJJJJJJJJJJJJJJJJFJFJJJJJJJJJJJ\tNM:i:0\tMD:Z:46\tAS:i:46\tXS:i:0\tSA:Z:OZ038621.1,1040569,-,39S86M26S,60,0;OZ038621.1,1301509,+,33M118S,60,0;\n".to_vec());
        data.append(&mut b"ERR4035126.621281\t2048\tOZ038621.1\t1301509\t60\t33M118H\t*\t0\t0\tGCCAGGGCGTCCAGTTTGTGCTGTGGCACGCCG\tAAFFFJJJJJJJJJJJJJJJJJJJJJJJJJJJJ\tNM:i:0\tMD:Z:33\tAS:i:33\tXS:i:0\tSA:Z:OZ038621.1,1040569,-,39S86M26S,60,0;OZ038621.1,3172373,-,46M105S,60,0;\n".to_vec());

        let expected = vec![
            PseudoAln{ query_id: Some(0), query_name: Some("ERR4035126.1".as_bytes().to_vec()), ones_names: Some(vec!["OZ038621.1".as_bytes().to_vec()]), ones: Some(vec![0]) },
            PseudoAln{ query_id: Some(1), query_name: Some("ERR4035126.2".as_bytes().to_vec()), ones_names: Some(vec!["OZ038621.1".as_bytes().to_vec()]), ones: Some(vec![0]) },
            PseudoAln{ query_id: Some(2), query_name: Some("ERR4035126.3".as_bytes().to_vec()), ones_names: Some(vec!["OZ038621.1".as_bytes().to_vec()]), ones: Some(vec![0]) },
            PseudoAln{ query_id: Some(3), query_name: Some("ERR4035126.1261584".as_bytes().to_vec()), ones_names: Some(vec!["OZ038622.1".as_bytes().to_vec()]), ones: Some(vec![1]) },
            PseudoAln{ query_id: Some(4), query_name: Some("ERR4035126.1213410".as_bytes().to_vec()), ones_names: Some(vec!["OZ038622.1".as_bytes().to_vec()]), ones: Some(vec![1]) },
            PseudoAln{ query_id: Some(4), query_name: Some("ERR4035126.1213410".as_bytes().to_vec()), ones_names: Some(vec!["OZ038621.1".as_bytes().to_vec()]), ones: Some(vec![0]) },
            PseudoAln{ query_id: Some(5), query_name: Some("ERR4035126.4".as_bytes().to_vec()), ones_names: Some(vec!["OZ038621.1".as_bytes().to_vec()]), ones: Some(vec![0]) },
            PseudoAln{ query_id: Some(6), query_name: Some("ERR4035126.5".as_bytes().to_vec()), ones_names: Some(vec!["OZ038621.1".as_bytes().to_vec()]), ones: Some(vec![0]) },
            PseudoAln{ query_id: Some(7), query_name: Some("ERR4035126.6".as_bytes().to_vec()), ones_names: Some(vec!["OZ038621.1".as_bytes().to_vec()]), ones: Some(vec![0]) },
            PseudoAln{ query_id: Some(8), query_name: Some("ERR4035126.973529".as_bytes().to_vec()), ones_names: Some(vec!["OZ038621.1".as_bytes().to_vec()]), ones: Some(vec![0]) },
            PseudoAln{ query_id: Some(8), query_name: Some("ERR4035126.973529".as_bytes().to_vec()), ones_names: Some(vec!["OZ038621.1".as_bytes().to_vec()]), ones: Some(vec![0]) },
            PseudoAln{ query_id: Some(9), query_name: Some("ERR4035126.621281".as_bytes().to_vec()), ones_names: Some(vec!["OZ038621.1".as_bytes().to_vec()]), ones: Some(vec![0]) },
            PseudoAln{ query_id: Some(10), query_name: Some("ERR4035126.1178767".as_bytes().to_vec()), ones_names: None, ones: None },
            PseudoAln{ query_id: Some(9), query_name: Some("ERR4035126.621281".as_bytes().to_vec()), ones_names: Some(vec!["OZ038621.1".as_bytes().to_vec()]), ones: Some(vec![0]) },
            PseudoAln{ query_id: Some(9), query_name: Some("ERR4035126.621281".as_bytes().to_vec()), ones_names: Some(vec!["OZ038621.1".as_bytes().to_vec()]), ones: Some(vec![0]) },
        ];

        let mut cursor: Cursor<Vec<u8>> = Cursor::new(data);

        let targets = vec!["OZ038621.1".as_bytes().to_vec(), "OZ038622.1".as_bytes().to_vec()];
        let queries = vec![
            "ERR4035126.1".as_bytes().to_vec(),
            "ERR4035126.2".as_bytes().to_vec(),
            "ERR4035126.3".as_bytes().to_vec(),
            "ERR4035126.1261584".as_bytes().to_vec(),
            "ERR4035126.1213410".as_bytes().to_vec(),
            "ERR4035126.4".as_bytes().to_vec(),
            "ERR4035126.5".as_bytes().to_vec(),
            "ERR4035126.6".as_bytes().to_vec(),
            "ERR4035126.973529".as_bytes().to_vec(),
            "ERR4035126.621281".as_bytes().to_vec(),
            "ERR4035126.1178767".as_bytes().to_vec(),
        ];
        let mut it = queries.into_iter();
        let mut t_it = targets.into_iter();
        let mut reader = Parser::new(&mut cursor, Some(&mut it), Some(&mut t_it)).unwrap();

        let mut res: Vec<PseudoAln> = Vec::new();
        for record in reader.by_ref() {
            res.push(record);
        }

        let (got, got_format) = (res, reader.format);

        assert_eq!(got_format, Format::SAM);

        got.iter().zip(expected.iter()).for_each(|(x, y)| { assert_eq!(x, y) });

        assert_eq!(got, expected);
    }

    #[test]
    fn parse_sam_output_with_targets_from_data() {
        use super::Parser;

        use crate::Format;
        use crate::PseudoAln;

        use std::io::Cursor;

        let mut data: Vec<u8> = b"@HD\tVN:1.5\tSO:unsorted\tGO:query\n".to_vec();
        data.append(&mut b"@SQ\tSN:OZ038621.1\tLN:5535987\n".to_vec());
        data.append(&mut b"@SQ\tSN:OZ038622.1\tLN:104814\n".to_vec());
        data.append(&mut b"@PG\tID:bwa\tPN:bwa\tVN:0.7.19-r1273\tCL:bwa mem -t 10 -o fwd_test.sam GCA_964037205.1_30348_1_60_genomic.fna ERR4035126_1.fastq.gz\n".to_vec());
        data.append(&mut b"ERR4035126.1\t16\tOZ038621.1\t4541508\t60\t151M\t*\t0\t0\tAGTATTTAGTGACCTAAGTCAATAAAATTTTAATTTACTCACGGCAGGTAACCAGTTCAGAAGCTGCTATCAGACACTCTTTTTTTAATCCACACAGAGACATATTGCCCGTTGCAGTCAGAATGAAAAGCTGAAAATCACTTACTAAGGC\tFJ<<JJFJAA<-JFAJFAF<JFFJJJJJJJFJFJJA<A<AJJAAAFFJJJJFJJFJFJAJJ7JJJJJFJJJJJFFJFFJFJJJJJJFJ7FFJAJJJJJJJJFJJFJJFJFJJJJFJJFJJJJJJJJJFFJJJJJJJJJJJJJFJJJFFAAA\tNM:i:0\tMD:Z:151\tAS:i:151\tXS:i:0\n".to_vec());
        data.append(&mut b"ERR4035126.2\t16\tOZ038621.1\t4541557\t60\t151M\t*\t0\t0\tAACCAGTTCAGAAGCTGCTATCAGACACTCTTTTTTTAATCCACACAGAGACATATTGCCCGTTGCAGTCAGAATGAAAAGCTGAAAATCACTTACTAAGGCGTTTTTTATTTGGTGATATTTTTTTCAATATCATGCAGCAAACGGTGCA\tJAFJFJJJFFJFAJJJJJJJJJJFFA<JJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJFJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJFJJJJJJJJJJJJJJJFF-FFFAA\tNM:i:0\tMD:Z:151\tAS:i:151\tXS:i:0\n".to_vec());
        data.append(&mut b"ERR4035126.3\t16\tOZ038621.1\t4541521\t60\t151M\t*\t0\t0\tCTAAGTCAATAAAATTTTAATTTACTCACGGCAGGTAACCAGTTCAGAAGCTGCTATCAGACACTCTTTTTTTAATCCACACAGAGACATATTGCCCGTTGCAGTCAGAATGAAAAGCTGAAAATCACTTACTAAGGCGTTTTTTATTTGG\tJJJJJJJFJFFFJJJJJJAJJJF7JJJJJ<JJFFJJJJJJJFJJJJJJJJJFFFJJJFJJJJJJJJJJJJJJJJAJFJJJJFJJJJJJJJJJJJJJJJJJJJJJAJJJJJJJJJJJJJJJJJAJFJFJJJJJJJJJJJJJJJJJFJFAFAA\tNM:i:0\tMD:Z:151\tAS:i:151\tXS:i:0\n".to_vec());
        data.append(&mut b"ERR4035126.1261584\t16\tOZ038622.1\t66398\t60\t151M\t*\t0\t0\tGCCGCTGTCTGAACCATGATATTGGCGGAACCGATGCCCATGATGGATGCGCCCCACAGCATGACCAGTTGCGCCAGACTCCAGCCGGAAGCGGTGGGCACAATCATCAAAAATCCACTCACGACACTGAGTATGCCGACGACGTCCCGTC\tFFJJJJFFJFJFFFJJJJJJJJJJJ7FA<JJ<JFJJFJJJJF-FJJA<FJJJJAJJJJJJJJJJJJJJJJJJJJFFJJJFJJJJJJJJJJJJJJJJJJFJFJJJJJJJJJFJJJJJFJJFJFJFJJJJJJJJJJJJJJJJJJJJJJFFFAA\tNM:i:0\tMD:Z:151\tAS:i:151\tXS:i:0\n".to_vec());
        data.append(&mut b"ERR4035126.1213410\t16\tOZ038622.1\t3996\t60\t151M\t*\t0\t0\tGCTGGCGCTTCGGGGATATGTGTTTCGACGGCAGATGAATTTATTCCGGCGGGGGCTGATTCTGCCGTCTGTTCAGTAAATACAGGTGCGATAATATCTGTTTTTTCGGATAAGGACGGTGGCGAAAAAGTACGACGTTTTTTCACCACAA\tJJJJJJJJJJJJJJJJJJJJJJJJJJJFJFJFFJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJFJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJFFJJJJJJJJJJJFFAAA\tNM:i:0\tMD:Z:151\tAS:i:151\tXS:i:0\n".to_vec());
        data.append(&mut b"ERR4035126.1213410\t16\tOZ038621.1\t3996\t60\t151M\t*\t0\t0\tGCTGGCGCTTCGGGGATATGTGTTTCGACGGCAGATGAATTTATTCCGGCGGGGGCTGATTCTGCCGTCTGTTCAGTAAATACAGGTGCGATAATATCTGTTTTTTCGGATAAGGACGGTGGCGAAAAAGTACGACGTTTTTTCACCACAA\tJJJJJJJJJJJJJJJJJJJJJJJJJJJFJFJFFJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJFJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJFFJJJJJJJJJJJFFAAA\tNM:i:0\tMD:Z:151\tAS:i:151\tXS:i:0\n".to_vec());
        data.append(&mut b"ERR4035126.4\t0\tOZ038621.1\t4541351\t60\t151M\t*\t0\t0\tAGGTGCGGGCTTTTTTCTGTGTTTCCTGTACGCGTCAGCCCGCACCGTTACCTGTGGTAATGGTGATGGTGGTGGTAATGGTGGTGCTAATGCGTTTCATGGATGTTGTGTACTCTGTAATTTTTATCTGTCTGTGCGCTATGCCTATATT\tAAFFFJJJJJJJJJJJJJJJJJJJJJFFJJJJJJJJJJJJJJJJJJJFFJJJJJJJJFJJJJJJJJJJJ<JFJJJJJJJJJJJAJJJFJJJJJJJJJJJJJJJJJJJFJJJJJJJJJJJJJJJJJJJJJJJFFFJFAFJJJJF<FFFJJJJ\tNM:i:0\tMD:Z:151\tAS:i:151\tXS:i:0\n".to_vec());
        data.append(&mut b"ERR4035126.5\t16\tOZ038621.1\t4541533\t60\t151M\t*\t0\t0\tAATTTTAATTTACTCACGGCAGGTAACCAGTTCAGAAGCTGCTATCAGACACTCTTTTTTTAATCCACACAGAGACATATTGCCCGTTGCAGTCAGAATGAAAAGCTGAAAATCACTTACTAAGGCGTTTTTTATTTGGTGATATTTTTTT\tFJJJJJJJJFJJJJJJJJFJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJFJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJFJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJFJJJJJJJJJJJJJFFFAA\tNM:i:0\tMD:Z:151\tAS:i:151\tXS:i:0\n".to_vec());
        data.append(&mut b"ERR4035126.6\t0\tOZ038621.1\t4541261\t60\t151M\t*\t0\t0\tTCTGCATTTGCCACTGATGTACCGCCGAACTTCAACACTCGCATGGTTGTTACCTCGTTACCTTTGGTCGAAAAAAAAGCCCGCACTGTCAGGTGCGGGCTTTTTTCTGTGTTTCCTGTACGCGTCAGCCCGCACCGTTACCTGTGGTAAT\tAAAFFJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJFJJFJJJJJJJJ<FJJJJJJJJJJJJJJJJJJJJJJJJJJJFJJJJJJJJFJJJJF7FJJJJJJJFJFJJJJJJJJFJJJJJJJJAJJJJFJFFFJFJF\tNM:i:0\tMD:Z:151\tAS:i:151\tXS:i:0\n".to_vec());
        data.append(&mut b"ERR4035126.973529\t16\tOZ038621.1\t3695316\t60\t66S85M\t*\t0\t0\tGGAGATGATTTCGTGTTTCTTCTCCGGGATGACCATGTCATCGATACCAACAGATGCACCAGAACGCGCCAAGTCGGGCAATCTGGTGAACTGGAAAGCCGGGGCGCTGTATCACCTGACGGAAAACGGCAATGTCTATATTAACTATGCC\tJJFJFF7-FFJJJA-FJFFFJJFAJJJJJJJJJJJJJJJJFJJJJJJJJFJJJJFJ<JJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJFFJFJJJJJJJJFJJJJJJJJFAJJJJJJJJJJJJJFJJJJJFFJJJFJJJJJJFJJFFFAA\tNM:i:0\tMD:Z:85\tAS:i:85\tXS:i:0\tSA:Z:OZ038621.1,5194124,-,69M82S,60,0;\n".to_vec());
        data.append(&mut b"ERR4035126.973529\t2064\tOZ038621.1\t5194124\t60\t69M82H\t*\t0\t0\tGGAGATGATTTCGTGTTTCTTCTCCGGGATGACCATGTCATCGATACCAACAGATGCACCAGAACGCGC\tJJFJFF7-FFJJJA-FJFFFJJFAJJJJJJJJJJJJJJJJFJJJJJJJJFJJJJFJ<JJJJJJJJJJJJ\tNM:i:0\tMD:Z:69\tAS:i:69\tXS:i:0\tSA:Z:OZ038621.1,3695316,-,66S85M,60,0;\n".to_vec());
        data.append(&mut b"ERR4035126.621281\t16\tOZ038621.1\t1040569\t60\t39S86M26S\t*\t0\t0\tGCTCGACCGCGTCCCAGTTGAAATGCAACTCCCCAGCCAACTCGATAAACACGATGATTAACACGGCAGTCATGGTCAGAATGGAAACGGGATCGAAAATCGGCATACCAAATGACATCGGCGTGCCACAGCACAAACTGGACGCCCTGGC\tAFAJJJJJJJJJJJJFJJJJJJJJJJJJJJJJFJFJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJFFFAA\tNM:i:0\tMD:Z:86\tAS:i:86\tXS:i:0\tSA:Z:OZ038621.1,3172373,-,46M105S,60,0;OZ038621.1,1301509,+,33M118S,60,0;\n".to_vec());
        data.append(&mut b"ERR4035126.1178767\t4\t*\t0\t0\t*\t*\t0\t0\tACTTGGCTCATGTTCCGTCAATGCCGGAGAGACAATTGAAGTTGATTTAGGTGATGTCTTCGCTGCCAATTTCCGTGTTGTAGGGCATAAACCTCTTGGGGCCAGAACGGCAGAACTTGCAATTCCAGTCAGGTGTAACACGGGAAACGCG\tAAFFFJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJFJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJFJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJ\tAS:i:0\tXS:i:0\n".to_vec());
        data.append(&mut b"ERR4035126.621281\t2064\tOZ038621.1\t3172373\t60\t46M105H\t*\t0\t0\tGCTCGACCGCGTCCCAGTTGAAATGCAACTCCCCAGCCAACTCGAT\tAFAJJJJJJJJJJJJFJJJJJJJJJJJJJJJJFJFJJJJJJJJJJJ\tNM:i:0\tMD:Z:46\tAS:i:46\tXS:i:0\tSA:Z:OZ038621.1,1040569,-,39S86M26S,60,0;OZ038621.1,1301509,+,33M118S,60,0;\n".to_vec());
        data.append(&mut b"ERR4035126.621281\t2048\tOZ038621.1\t1301509\t60\t33M118H\t*\t0\t0\tGCCAGGGCGTCCAGTTTGTGCTGTGGCACGCCG\tAAFFFJJJJJJJJJJJJJJJJJJJJJJJJJJJJ\tNM:i:0\tMD:Z:33\tAS:i:33\tXS:i:0\tSA:Z:OZ038621.1,1040569,-,39S86M26S,60,0;OZ038621.1,3172373,-,46M105S,60,0;\n".to_vec());

        let expected = vec![
            PseudoAln{ query_id: Some(0), query_name: Some("ERR4035126.1".as_bytes().to_vec()), ones_names: Some(vec!["OZ038621.1".as_bytes().to_vec()]), ones: Some(vec![0]) },
            PseudoAln{ query_id: Some(1), query_name: Some("ERR4035126.2".as_bytes().to_vec()), ones_names: Some(vec!["OZ038621.1".as_bytes().to_vec()]), ones: Some(vec![0]) },
            PseudoAln{ query_id: Some(2), query_name: Some("ERR4035126.3".as_bytes().to_vec()), ones_names: Some(vec!["OZ038621.1".as_bytes().to_vec()]), ones: Some(vec![0]) },
            PseudoAln{ query_id: Some(3), query_name: Some("ERR4035126.1261584".as_bytes().to_vec()), ones_names: Some(vec!["OZ038622.1".as_bytes().to_vec()]), ones: Some(vec![1]) },
            PseudoAln{ query_id: Some(4), query_name: Some("ERR4035126.1213410".as_bytes().to_vec()), ones_names: Some(vec!["OZ038622.1".as_bytes().to_vec()]), ones: Some(vec![1]) },
            PseudoAln{ query_id: Some(4), query_name: Some("ERR4035126.1213410".as_bytes().to_vec()), ones_names: Some(vec!["OZ038621.1".as_bytes().to_vec()]), ones: Some(vec![0]) },
            PseudoAln{ query_id: Some(5), query_name: Some("ERR4035126.4".as_bytes().to_vec()), ones_names: Some(vec!["OZ038621.1".as_bytes().to_vec()]), ones: Some(vec![0]) },
            PseudoAln{ query_id: Some(6), query_name: Some("ERR4035126.5".as_bytes().to_vec()), ones_names: Some(vec!["OZ038621.1".as_bytes().to_vec()]), ones: Some(vec![0]) },
            PseudoAln{ query_id: Some(7), query_name: Some("ERR4035126.6".as_bytes().to_vec()), ones_names: Some(vec!["OZ038621.1".as_bytes().to_vec()]), ones: Some(vec![0]) },
            PseudoAln{ query_id: Some(8), query_name: Some("ERR4035126.973529".as_bytes().to_vec()), ones_names: Some(vec!["OZ038621.1".as_bytes().to_vec()]), ones: Some(vec![0]) },
            PseudoAln{ query_id: Some(8), query_name: Some("ERR4035126.973529".as_bytes().to_vec()), ones_names: Some(vec!["OZ038621.1".as_bytes().to_vec()]), ones: Some(vec![0]) },
            PseudoAln{ query_id: Some(9), query_name: Some("ERR4035126.621281".as_bytes().to_vec()), ones_names: Some(vec!["OZ038621.1".as_bytes().to_vec()]), ones: Some(vec![0]) },
            PseudoAln{ query_id: Some(10), query_name: Some("ERR4035126.1178767".as_bytes().to_vec()), ones_names: None, ones: None },
            PseudoAln{ query_id: Some(9), query_name: Some("ERR4035126.621281".as_bytes().to_vec()), ones_names: Some(vec!["OZ038621.1".as_bytes().to_vec()]), ones: Some(vec![0]) },
            PseudoAln{ query_id: Some(9), query_name: Some("ERR4035126.621281".as_bytes().to_vec()), ones_names: Some(vec!["OZ038621.1".as_bytes().to_vec()]), ones: Some(vec![0]) },
        ];

        let mut cursor: Cursor<Vec<u8>> = Cursor::new(data);

        let queries = vec![
            "ERR4035126.1".as_bytes().to_vec(),
            "ERR4035126.2".as_bytes().to_vec(),
            "ERR4035126.3".as_bytes().to_vec(),
            "ERR4035126.1261584".as_bytes().to_vec(),
            "ERR4035126.1213410".as_bytes().to_vec(),
            "ERR4035126.4".as_bytes().to_vec(),
            "ERR4035126.5".as_bytes().to_vec(),
            "ERR4035126.6".as_bytes().to_vec(),
            "ERR4035126.973529".as_bytes().to_vec(),
            "ERR4035126.621281".as_bytes().to_vec(),
            "ERR4035126.1178767".as_bytes().to_vec(),
        ];
        let mut it = queries.into_iter();
        let mut reader = Parser::new(&mut cursor, Some(&mut it),  None::<&mut std::iter::Empty<Vec<u8>>>).unwrap();

        let mut res: Vec<PseudoAln> = Vec::new();
        for record in reader.by_ref() {
            res.push(record);
        }

        let (got, got_format) = (res, reader.format);

        assert_eq!(got_format, Format::SAM);

        got.iter().zip(expected.iter()).for_each(|(x, y)| { assert_eq!(x, y) });

        assert_eq!(got, expected);
    }

}
