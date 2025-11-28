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
pub mod bifrost;
pub mod fulgor;
pub mod metagraph;
pub mod sam;
pub mod themisto;

use crate::Format;
use crate::PseudoAln;
use crate::headers::file::FileFlags;
use crate::headers::file::FileHeader;

use crate::parser::bifrost::read_bifrost;
use crate::parser::fulgor::read_fulgor;
use crate::parser::metagraph::read_metagraph;
use crate::parser::sam::read_sam;
use crate::parser::themisto::read_themisto;

use std::collections::HashMap;
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

pub struct Parser<'a, R: Read> {
    reader: BufReader<&'a mut R>,
    buf: Cursor<Vec<u8>>,
    pub format: Format,

    query_to_pos: HashMap<String, usize>,
    pos_to_query: HashMap<usize, String>,
    target_to_pos: HashMap<String, usize>,

    header: FileHeader,
    flags: FileFlags,

}

impl<'a, R: Read> Parser<'a, R> {
    pub fn new(
        conn: &'a mut R,
        targets: &[String],
        queries: &[String],
        sample_name: &str,
    ) -> Result<Self, E> {

        // TODO Don't add keys twice to a hashmap if present

        let mut query_to_pos: HashMap<String, usize> = HashMap::new();
        let mut pos_to_query: HashMap<usize, String> = HashMap::new();
        queries.iter().enumerate().for_each(|(idx, query)| {
            query_to_pos.insert(query.clone(), idx);
            pos_to_query.insert(idx, query.clone());
        });

        let mut target_to_pos: HashMap<String, usize> = HashMap::new();
        targets.iter().enumerate().for_each(|(idx, target)| {
            target_to_pos.insert(target.clone(), idx);
        });

        let flags = FileFlags{ target_names: targets.to_vec(), query_name: sample_name.to_string() };
        let flags_bytes = crate::headers::file::encode_file_flags(&flags).unwrap();
        let header = FileHeader{ n_targets: targets.len() as u32, n_queries: query_to_pos.len() as u32, flags_len: flags_bytes.len() as u32, format: 1_u16, ph2: 0, ph3: 0, ph4: 0 };

        let mut reader = BufReader::new(conn);
        let mut buf = Cursor::new(Vec::<u8>::new());

        reader.read_until(b'\n', buf.get_mut())?;

        if let Some(format) = guess_format(buf.get_ref()) {
            Ok(Self {
                reader, buf, format,
                query_to_pos, pos_to_query, target_to_pos,
                header, flags,
            })
        } else {
            Err(Box::new(UnrecognizedInputFormat{}))
        }

    }

}

impl<R: Read> Parser<'_, R> {
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
            Format::Metagraph => None,
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
            Format::SAM => {
                // TODO Error if the header is misformatted
                let mut header_contents = Cursor::new(self.buf.get_mut().clone());
                let mut next_line: Cursor<Vec<u8>> = Cursor::new(Vec::new());
                loop {
                    self.reader.read_until(b'\n', next_line.get_mut()).unwrap();
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
                let mut reader = noodles_sam::io::reader::Builder::default().build_from_reader(&mut header_contents).unwrap();
                let header = reader.read_header().unwrap();
                let target_names: Vec<String> = header.reference_sequences().iter().map(|x| x.0.to_string()).collect();
                Some(target_names)
            },
        }
    }

    pub fn file_header(
        &self
    ) -> &FileHeader {
        &self.header
    }

    pub fn file_flags(
        &self
    ) -> &FileFlags {
        &self.flags
    }
}

impl<R: Read> Iterator for Parser<'_, R> {
    type Item = PseudoAln;

    fn next(
        &mut self,
    ) -> Option<PseudoAln> {
        let mut line = Cursor::new(Vec::<u8>::new());
        let record = if !self.buf.get_ref().is_empty() {
            line = self.buf.clone();
            if line.get_mut().contains(&b'\n') {
                line.get_mut().pop();
            }
            let record = match self.format {
                Format::Themisto => read_themisto(&mut line).unwrap(),
                Format::Fulgor => read_fulgor(&mut line).unwrap(),
                Format::Metagraph => read_metagraph(&mut line).unwrap(),
                Format::Bifrost => {
                    let _ = self.read_header();

                    line.get_mut().clear();
                    self.reader.read_until(b'\n', line.get_mut()).unwrap();
                    line.get_mut().pop();
                    read_bifrost(&mut line).unwrap()
                },
                Format::SAM => {
                    let _ = self.read_header();
                    self.buf.get_mut().pop(); // first line after header is now here
                    read_sam(&mut self.buf).unwrap()
                },
            };
            self.buf.get_mut().clear();
            Some(record)
        } else if self.reader.read_until(b'\n', line.get_mut()).is_ok() {
            if line.get_mut().is_empty() {
                return None
            }
            line.get_mut().pop();
            Some(
                match self.format {
                    Format::Themisto => read_themisto(&mut line).unwrap(),
                    Format::Fulgor => read_fulgor(&mut line).unwrap(),
                    Format::Metagraph => read_metagraph(&mut line).unwrap(),
                    Format::Bifrost => read_bifrost(&mut line).unwrap(),
                    Format::SAM => read_sam(&mut line).unwrap(),
                },
            )
        } else {
            None
        };

        let mut record = record?;
        record.query_id = if record.query_id.is_some() { record.query_id } else { Some(*self.query_to_pos.get(&record.query_name.clone().unwrap()).unwrap() as u32) };
        record.query_name = if record.query_name.is_some() { record.query_name } else { Some(self.pos_to_query.get(&(record.query_id.unwrap() as usize)).unwrap().clone()) };
        if record.ones.is_some() {
            record.ones_names = if record.ones_names.is_some() { record.ones_names } else {
                Some(record.ones.as_ref().unwrap().iter().map(|target_idx| {
                    self.flags.target_names[*target_idx as usize].clone()
                }).collect::<Vec<String>>())};
        }
        if record.ones_names.is_some() {
            record.ones = Some(
                record.ones_names.as_ref().unwrap().iter().map(|target_name| {
                    *self.target_to_pos.get(&target_name.clone()).unwrap() as u32
                }).collect::<Vec<u32>>()
            );
        }

        Some(record)
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

    if bytes.len() > 2 {
        let sam: bool = bytes[0] == b'@' && bytes[1] == b'H' && bytes[2] == b'D';
        if sam {
            return Some(Format::SAM)
        }
    }

    let not_themisto: bool = first_line.contains(&b'\t');
    if !not_themisto {
        return Some(Format::Themisto)
    }

    let line = first_line.clone().iter().map(|x| *x as char).collect::<String>();
    let mut records = line.split('\t');

    let first_record = records.next()?;
    let bifrost: bool = first_record == "query_name";
    if bifrost {
        return Some(Format::Bifrost)
    }

    let maybe_metagraph: bool = first_record.parse::<u32>().is_ok();

    let next = records.next()?;

    let fulgor: bool = next.parse::<u32>().is_ok();

    if fulgor && maybe_metagraph {
        return None
    }

    if fulgor {
        return Some(Format::Fulgor)
    }

    if maybe_metagraph {
        return Some(Format::Metagraph)
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
        let expected: PseudoAln = PseudoAln{ones_names: Some(vec!["chr.fasta".to_string()]),  query_id: Some(0), ones: Some(vec![0]), query_name: Some("ERR4035126.1".to_string()) };

        let mut cursor = Cursor::new(data);

        let targets = vec!["chr.fasta".to_string(), "plasmid.fasta".to_string()];
        let queries = vec!["ERR4035126.1".to_string()];
        let sample_name = "ERR4035126";
        let mut reader = Parser::new(&mut cursor, &targets, &queries, &sample_name).unwrap();

        let got: PseudoAln = reader.next().unwrap();

        assert_eq!(got, expected);
    }

    #[test]
    fn read_bifrost_header() {
        use super::Parser;
        use std::io::Cursor;

        let mut data: Vec<u8> = b"query_name\tchr.fasta\tplasmid.fasta\n".to_vec();
        data.append(&mut b"ERR4035126.1\t121\t0\n".to_vec());
        let expected: Vec<String> = vec!["chr.fasta".to_string(), "plasmid.fasta".to_string()];

        let mut cursor = Cursor::new(data);

        let targets = vec!["chr.fasta".to_string(), "plasmid.fasta".to_string()];
        let queries = vec!["ERR4035126.1".to_string()];
        let sample_name = "ERR4035126";
        let mut reader = Parser::new(&mut cursor, &targets, &queries, &sample_name).unwrap();

        let got: Vec<String> = reader.read_header().unwrap();

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

        let expected: Vec<String> = vec!["OZ038621.1".to_string(), "OZ038622.1".to_string()];

        let mut cursor = Cursor::new(data);

        let targets = vec!["OZ038621.1".to_string(), "OZ038622.1".to_string()];
        let queries = vec!["ERR4035126.1".to_string()];
        let sample_name = "ERR4035126";
        let mut reader = Parser::new(&mut cursor, &targets, &queries, &sample_name).unwrap();

        let got: Vec<String> = reader.read_header().unwrap();

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

        let expected = PseudoAln{ones_names: Some(vec!["OZ038621.1".to_string()]), query_id: Some(0), ones: Some(vec![0]), query_name: Some("ERR4035126.1".to_string()) };

        let mut cursor = Cursor::new(data);

        let targets = vec!["OZ038621.1".to_string(), "OZ038622.1".to_string()];
        let queries = vec!["ERR4035126.1".to_string()];
        let sample_name = "ERR4035126";
        let mut reader = Parser::new(&mut cursor, &targets, &queries, &sample_name).unwrap();

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

        let expected_header: Vec<String> = vec!["OZ038621.1".to_string(), "OZ038622.1".to_string()];
        let expected_aln = PseudoAln{ones_names: Some(vec!["OZ038621.1".to_string()]), query_id: Some(0), ones: Some(vec![0]), query_name: Some("ERR4035126.1".to_string()) };

        let mut cursor = Cursor::new(data);

        let targets = vec!["OZ038621.1".to_string(), "OZ038622.1".to_string()];
        let queries = vec!["ERR4035126.1".to_string()];
        let sample_name = "ERR4035126";
        let mut reader = Parser::new(&mut cursor, &targets, &queries, &sample_name).unwrap();

        let got_header: Vec<String> = reader.read_header().unwrap();
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
            PseudoAln{ones_names: Some(vec!["OZ038621.1".to_string()]), query_id: Some(0), ones: Some(vec![0]), query_name: Some("ERR4035126.1".to_string()) },
            PseudoAln{ones_names: Some(vec!["OZ038621.1".to_string()]), query_id: Some(1), ones: Some(vec![0]), query_name: Some("ERR4035126.2".to_string()) },
            PseudoAln{ones_names: Some(vec!["OZ038622.1".to_string()]), query_id: Some(2), ones: Some(vec![1]), query_name: Some("ERR4035126.3".to_string()) },
        ];

        let mut cursor = Cursor::new(data);

        let targets = vec!["OZ038621.1".to_string(), "OZ038622.1".to_string()];
        let queries = vec!["ERR4035126.1".to_string(), "ERR4035126.2".to_string(), "ERR4035126.3".to_string()];
        let sample_name = "ERR4035126";
        let mut reader = Parser::new(&mut cursor, &targets, &queries, &sample_name).unwrap();

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
            PseudoAln{ones_names: Some(vec!["0".to_string(), "7".to_string(), "11".to_string(), "3".to_string()]),  query_id: Some(128), ones: Some(vec![0, 7, 11, 3]), query_name: Some("128".to_string())},
            PseudoAln{ones_names: Some(vec!["3".to_string(), "2".to_string(), "1".to_string(), "0".to_string()]),  query_id: Some(7),   ones: Some(vec![3, 2, 1, 0]), query_name: Some("7".to_string())},
            PseudoAln{ones_names: Some(vec![]),  query_id: Some(8),   ones: Some(vec![]), query_name: Some("8".to_string())},
            PseudoAln{ones_names: Some(vec![]),  query_id: Some(0),   ones: Some(vec![]), query_name: Some("0".to_string())},
            PseudoAln{ones_names: Some(vec!["4".to_string(), "2".to_string(), "9".to_string(), "7".to_string()]),  query_id: Some(1),   ones: Some(vec![4, 2, 9, 7]), query_name: Some("1".to_string())},
        ];

        let mut cursor: Cursor<Vec<u8>> = Cursor::new(data);

        let targets = vec![
            "0".to_string(),
            "1".to_string(),
            "2".to_string(),
            "3".to_string(),
            "4".to_string(),
            "5".to_string(),
            "6".to_string(),
            "7".to_string(),
            "8".to_string(),
            "9".to_string(),
            "10".to_string(),
            "11".to_string(),
        ];
        let queries = (0..129).map(|x| x.to_string()).collect::<Vec<String>>();
        let sample_name = "sample";
        let mut reader = Parser::new(&mut cursor, &targets, &queries, &sample_name).unwrap();

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
            PseudoAln{ones_names: Some(vec![]),  query_id: Some(0), ones: Some(vec![]), query_name: Some("ERR4035126.4996".to_string()) },
            PseudoAln{ones_names: Some(vec!["chr.fasta".to_string()]),  query_id: Some(1), ones: Some(vec![0]), query_name: Some("ERR4035126.1262953".to_string()) },
            PseudoAln{ones_names: Some(vec!["plasmid.fasta".to_string()]),  query_id: Some(2), ones: Some(vec![1]), query_name: Some("ERR4035126.1262954".to_string()) },
            PseudoAln{ones_names: Some(vec!["plasmid.fasta".to_string()]),  query_id: Some(3), ones: Some(vec![1]), query_name: Some("ERR4035126.1262955".to_string()) },
            PseudoAln{ones_names: Some(vec!["chr.fasta".to_string()]),  query_id: Some(4), ones: Some(vec![0]), query_name: Some("ERR4035126.1262956".to_string()) },
            PseudoAln{ones_names: Some(vec!["chr.fasta".to_string()]),  query_id: Some(5), ones: Some(vec![0]), query_name: Some("ERR4035126.1262957".to_string()) },
            PseudoAln{ones_names: Some(vec!["chr.fasta".to_string()]),  query_id: Some(6), ones: Some(vec![0]), query_name: Some("ERR4035126.1262958".to_string()) },
            PseudoAln{ones_names: Some(vec!["chr.fasta".to_string()]),  query_id: Some(7), ones: Some(vec![0]), query_name: Some("ERR4035126.1262959".to_string()) },
            PseudoAln{ones_names: Some(vec!["chr.fasta".to_string(), "plasmid.fasta".to_string()]),  query_id: Some(8), ones: Some(vec![0, 1]), query_name: Some("ERR4035126.651965".to_string()) },
            PseudoAln{ones_names: Some(vec![]),  query_id: Some(9), ones: Some(vec![]), query_name: Some("ERR4035126.11302".to_string()) },
            PseudoAln{ones_names: Some(vec!["chr.fasta".to_string()]),  query_id: Some(10), ones: Some(vec![0]), query_name: Some("ERR4035126.1262960".to_string()) },
            PseudoAln{ones_names: Some(vec!["chr.fasta".to_string()]),  query_id: Some(11), ones: Some(vec![0]), query_name: Some("ERR4035126.1262961".to_string()) },
            PseudoAln{ones_names: Some(vec!["chr.fasta".to_string()]),  query_id: Some(12), ones: Some(vec![0]), query_name: Some("ERR4035126.1262962".to_string()) },
            PseudoAln{ones_names: Some(vec!["chr.fasta".to_string(), "plasmid.fasta".to_string()]),  query_id: Some(8), ones: Some(vec![0, 1]), query_name: Some("ERR4035126.651965".to_string()) },
        ];

        let mut cursor: Cursor<Vec<u8>> = Cursor::new(data);

        let targets = vec!["chr.fasta".to_string(), "plasmid.fasta".to_string()];
        let queries = vec![
            "ERR4035126.4996".to_string(),
            "ERR4035126.1262953".to_string(),
            "ERR4035126.1262954".to_string(),
            "ERR4035126.1262955".to_string(),
            "ERR4035126.1262956".to_string(),
            "ERR4035126.1262957".to_string(),
            "ERR4035126.1262958".to_string(),
            "ERR4035126.1262959".to_string(),
            "ERR4035126.651965".to_string(),
            "ERR4035126.11302".to_string(),
            "ERR4035126.1262960".to_string(),
            "ERR4035126.1262961".to_string(),
            "ERR4035126.1262962".to_string(),
        ];
        let sample_name = "ERR4035126";
        let mut reader = Parser::new(&mut cursor, &targets, &queries, &sample_name).unwrap();

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
            PseudoAln{ query_name: Some("ERR4035126.724962".to_string()), ones: Some(vec![]), ones_names: Some(vec![]), query_id: Some(0) },
            PseudoAln{ query_name: Some("ERR4035126.1235744".to_string()), ones: Some(vec![]), ones_names: Some(vec![]), query_id: Some(1) },
            PseudoAln{ query_name: Some("ERR4035126.431001".to_string()), ones: Some(vec![]), ones_names: Some(vec![]), query_id: Some(2) },
            PseudoAln{ query_name: Some("ERR4035126.645400".to_string()), ones: Some(vec![]), ones_names: Some(vec![]), query_id: Some(3) },
            PseudoAln{ query_name: Some("ERR4035126.3001".to_string()), ones: Some(vec![0]), ones_names: Some(vec!["chromosome.fasta".to_string()]), query_id: Some(4) },
            PseudoAln{ query_name: Some("ERR4035126.515778".to_string()), ones: Some(vec![0]), ones_names: Some(vec!["chromosome.fasta".to_string()]), query_id: Some(5) },
            PseudoAln{ query_name: Some("ERR4035126.886205".to_string()), ones: Some(vec![0]), ones_names: Some(vec!["chromosome.fasta".to_string()]), query_id: Some(6) },
            PseudoAln{ query_name: Some("ERR4035126.1254676".to_string()), ones: Some(vec![0]), ones_names: Some(vec!["chromosome.fasta".to_string()]), query_id: Some(7) },
            PseudoAln{ query_name: Some("ERR4035126.668031".to_string()), ones: Some(vec![1]), ones_names: Some(vec!["plasmid.fasta".to_string()]), query_id: Some(8) },
            PseudoAln{ query_name: Some("ERR4035126.388619".to_string()), ones: Some(vec![0]), ones_names: Some(vec!["chromosome.fasta".to_string()]), query_id: Some(9) },
            PseudoAln{ query_name: Some("ERR4035126.959743".to_string()), ones: Some(vec![]), ones_names: Some(vec![]), query_id: Some(10) },
            PseudoAln{ query_name: Some("ERR4035126.1146685".to_string()), ones: Some(vec![]), ones_names: Some(vec![]), query_id: Some(11) },
            PseudoAln{ query_name: Some("ERR4035126.1017809".to_string()), ones: Some(vec![]), ones_names: Some(vec![]), query_id: Some(12) },
            PseudoAln{ query_name: Some("ERR4035126.788136".to_string()), ones: Some(vec![]), ones_names: Some(vec![]), query_id: Some(13) },
            PseudoAln{ query_name: Some("ERR4035126.1223924".to_string()), ones: Some(vec![0, 1]), ones_names: Some(vec!["chromosome.fasta".to_string(), "plasmid.fasta".to_string()]), query_id: Some(14) },
            PseudoAln{ query_name: Some("ERR4035126.910807".to_string()), ones: Some(vec![]), ones_names: Some(vec![]), query_id: Some(15) },
            PseudoAln{ query_name: Some("ERR4035126.824748".to_string()), ones: Some(vec![0]), ones_names: Some(vec!["chromosome.fasta".to_string()]), query_id: Some(16) },
        ];

        let mut cursor: Cursor<Vec<u8>> = Cursor::new(data);

        let targets = vec!["chromosome.fasta".to_string(), "plasmid.fasta".to_string()];
        let queries = vec![
            "ERR4035126.724962".to_string(),
            "ERR4035126.1235744".to_string(),
            "ERR4035126.431001".to_string(),
            "ERR4035126.645400".to_string(),
            "ERR4035126.3001".to_string(),
            "ERR4035126.515778".to_string(),
            "ERR4035126.886205".to_string(),
            "ERR4035126.1254676".to_string(),
            "ERR4035126.668031".to_string(),
            "ERR4035126.388619".to_string(),
            "ERR4035126.959743".to_string(),
            "ERR4035126.1146685".to_string(),
            "ERR4035126.1017809".to_string(),
            "ERR4035126.788136".to_string(),
            "ERR4035126.1223924".to_string(),
            "ERR4035126.910807".to_string(),
            "ERR4035126.824748".to_string(),
        ];
        let sample_name = "ERR4035126";
        let mut reader = Parser::new(&mut cursor, &targets, &queries, &sample_name).unwrap();

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
            PseudoAln{ones_names: Some(vec!["chr.fasta".to_string()]),  query_id: Some(3), ones: Some(vec![]), query_name: Some("ERR4035126.2".to_string()) },
            PseudoAln{ones_names: Some(vec!["chr.fasta".to_string()]),  query_id: Some(2), ones: Some(vec![]), query_name: Some("ERR4035126.1".to_string()) },
            PseudoAln{ones_names: Some(vec!["chr.fasta".to_string(), "plasmid.fasta".to_string()]),  query_id: Some(1303804), ones: Some(vec![]), query_name: Some("ERR4035126.651903".to_string()) },
            PseudoAln{ones_names: Some(vec![]),  query_id: Some(30), ones: Some(vec![]), query_name: Some("ERR4035126.16".to_string()) },
            PseudoAln{ones_names: Some(vec!["plasmid.fasta".to_string()]),  query_id: Some(15084), ones: Some(vec![]), query_name: Some("ERR4035126.7543".to_string()) },
        ];

        let mut cursor: Cursor<Vec<u8>> = Cursor::new(data);

        let targets = vec!["chr.fasta".to_string(), "plasmid.fasta".to_string()];
        let queries = vec![
            "ERR4035126.2".to_string(),
            "ERR4035126.1".to_string(),
            "ERR4035126.651903".to_string(),
            "ERR4035126.16".to_string(),
            "ERR4035126.7543".to_string(),
        ];
        let sample_name = "ERR4035126";
        let mut reader = Parser::new(&mut cursor, &targets, &queries, &sample_name).unwrap();

        let mut res: Vec<PseudoAln> = Vec::new();
        for record in reader.by_ref() {
            res.push(record);
        }

        let (got, got_format) = (res, reader.format);

        assert_eq!(got_format, Format::Metagraph);
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
            PseudoAln{ query_id: Some(0), query_name: Some("ERR4035126.1".to_string()), ones_names: Some(vec!["OZ038621.1".to_string()]), ones: Some(vec![0]) },
            PseudoAln{ query_id: Some(1), query_name: Some("ERR4035126.2".to_string()), ones_names: Some(vec!["OZ038621.1".to_string()]), ones: Some(vec![0]) },
            PseudoAln{ query_id: Some(2), query_name: Some("ERR4035126.3".to_string()), ones_names: Some(vec!["OZ038621.1".to_string()]), ones: Some(vec![0]) },
            PseudoAln{ query_id: Some(3), query_name: Some("ERR4035126.1261584".to_string()), ones_names: Some(vec!["OZ038622.1".to_string()]), ones: Some(vec![1]) },
            PseudoAln{ query_id: Some(4), query_name: Some("ERR4035126.1213410".to_string()), ones_names: Some(vec!["OZ038622.1".to_string()]), ones: Some(vec![1]) },
            PseudoAln{ query_id: Some(4), query_name: Some("ERR4035126.1213410".to_string()), ones_names: Some(vec!["OZ038621.1".to_string()]), ones: Some(vec![0]) },
            PseudoAln{ query_id: Some(5), query_name: Some("ERR4035126.4".to_string()), ones_names: Some(vec!["OZ038621.1".to_string()]), ones: Some(vec![0]) },
            PseudoAln{ query_id: Some(6), query_name: Some("ERR4035126.5".to_string()), ones_names: Some(vec!["OZ038621.1".to_string()]), ones: Some(vec![0]) },
            PseudoAln{ query_id: Some(7), query_name: Some("ERR4035126.6".to_string()), ones_names: Some(vec!["OZ038621.1".to_string()]), ones: Some(vec![0]) },
            PseudoAln{ query_id: Some(8), query_name: Some("ERR4035126.973529".to_string()), ones_names: Some(vec!["OZ038621.1".to_string()]), ones: Some(vec![0]) },
            PseudoAln{ query_id: Some(8), query_name: Some("ERR4035126.973529".to_string()), ones_names: Some(vec!["OZ038621.1".to_string()]), ones: Some(vec![0]) },
            PseudoAln{ query_id: Some(9), query_name: Some("ERR4035126.621281".to_string()), ones_names: Some(vec!["OZ038621.1".to_string()]), ones: Some(vec![0]) },
            PseudoAln{ query_id: Some(10), query_name: Some("ERR4035126.1178767".to_string()), ones_names: None, ones: None },
            PseudoAln{ query_id: Some(9), query_name: Some("ERR4035126.621281".to_string()), ones_names: Some(vec!["OZ038621.1".to_string()]), ones: Some(vec![0]) },
            PseudoAln{ query_id: Some(9), query_name: Some("ERR4035126.621281".to_string()), ones_names: Some(vec!["OZ038621.1".to_string()]), ones: Some(vec![0]) },
        ];

        let mut cursor: Cursor<Vec<u8>> = Cursor::new(data);

        let targets = vec!["OZ038621.1".to_string(), "OZ038622.1".to_string()];
        let queries = vec![
            "ERR4035126.1".to_string(),
            "ERR4035126.2".to_string(),
            "ERR4035126.3".to_string(),
            "ERR4035126.1261584".to_string(),
            "ERR4035126.1213410".to_string(),
            "ERR4035126.4".to_string(),
            "ERR4035126.5".to_string(),
            "ERR4035126.6".to_string(),
            "ERR4035126.973529".to_string(),
            "ERR4035126.621281".to_string(),
            "ERR4035126.1178767".to_string(),
        ];
        let sample_name = "ERR4035126";
        let mut reader = Parser::new(&mut cursor, &targets, &queries, &sample_name).unwrap();

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
