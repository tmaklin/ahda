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

//! Printer for outputting [PseudoAln] records as plain text in any supported [Format].
//!
//! Can be used to convert any iterator over [PseudoAln] data to their plain
//! text representation.
//!
//! Returns 1 line at a time using next().
//!
//! If the desired output format has header lines, this can be formatted by
//! Printer using [print_header](Printer::print_header).
//!
//! ## Usage
//!
//! ### Print PseudoAln records stored in memory
//!
//! ```rust
//! use ahda::{Format, PseudoAln};
//! use ahda::printer::Printer;
//! use std::io::{Cursor, Write};
//!
//! let targets = vec!["chr.fasta".to_string(), "plasmid.fasta".to_string(), "virus.fasta".to_string()];
//! let queries = vec!["r1".to_string(), "r2".to_string(), "r651903".to_string(), "r7543".to_string(), "r16".to_string()];
//! let name = "sample".to_string();
//!
//! let data = vec![
//!                 PseudoAln { ones: Some(vec![2]), ones_names: Some(vec!["virus.fasta".to_string()]), query_id: Some(0), query_name: Some("r1".to_string()) },
//!                 PseudoAln { ones: Some(vec![0, 2]), ones_names: Some(vec!["chr.fasta".to_string(), "virus.fasta".to_string()]), query_id: Some(3), query_name: Some("r7543".to_string()) },
//!                 PseudoAln { ones: Some(vec![0, 1, 2]), ones_names: Some(vec!["chr.fasta".to_string(), "plasmid.fasta".to_string(), "virus.fasta".to_string()]), query_id: Some(4), query_name: Some("r16".to_string()) },
//!                 PseudoAln { ones: Some(vec![]), ones_names: Some(vec![]), query_id: Some(2), query_name: Some("r651903".to_string()) }
//!                ];
//!
//! let mut iter = data.into_iter(); // Printer expectes PseudoAln, not &PseudoAln
//!
//! let mut printer = Printer::new(&mut iter, &targets, &queries, &name, Format::Metagraph);
//!
//! // Print the records in Metagraph format
//! let mut output: Cursor<Vec<u8>> = Cursor::new(Vec::new());
//! for line in printer.by_ref() {
//!     output.write_all(&line).unwrap()
//! }
//!
//! // Expect this plain text output
//! //   0    r1       virus.fasta
//! //   3    r7543    chr.fasta:virus.fasta
//! //   4    r16      chr.fasta:plasmid.fasta:virus.fasta
//! //   2    r651903
//! //
//! let mut expected: Vec<u8> = Vec::new();
//! expected.append(&mut b"0\tr1\tvirus.fasta\n".to_vec());
//! expected.append(&mut b"3\tr7543\tchr.fasta:virus.fasta\n".to_vec());
//! expected.append(&mut b"4\tr16\tchr.fasta:plasmid.fasta:virus.fasta\n".to_vec());
//! expected.append(&mut b"2\tr651903\t\n".to_vec());
//!
//! assert_eq!(output.get_ref(), &expected);
//! ```
//!
//! ### Print encoded PseudoAln records
//!
//! Initialize a [Decoder](ahda::decoder::Decoder) on the encoded bytes and pass
//! this to a Printer to print all records.
//!
//! ```rust
//! use ahda::decoder::Decoder;
//! use ahda::encoder::Encoder;
//! use ahda::printer::Printer;
//! use ahda::{Format, PseudoAln};
//! use std::io::{Cursor, Seek, Write};
//!
//! // Set up some encoded data
//! let targets = vec!["chr.fasta".to_string(), "plasmid.fasta".to_string(), "virus.fasta".to_string()];
//! let queries = vec!["r1".to_string(), "r2".to_string(), "r651903".to_string(), "r7543".to_string(), "r16".to_string()];
//! let name = "sample".to_string();
//!
//! let data: Vec<PseudoAln> = vec![
//!                                 PseudoAln { ones: Some(vec![2]), ones_names: Some(vec!["virus.fasta".to_string()]), query_id: Some(0), query_name: Some("r1".to_string()) },
//!                                 PseudoAln { ones: Some(vec![0, 2]), ones_names: Some(vec!["chr.fasta".to_string(), "virus.fasta".to_string()]), query_id: Some(3), query_name: Some("r7543".to_string()) },
//!                                 PseudoAln { ones: Some(vec![0, 1, 2]), ones_names: Some(vec!["chr.fasta".to_string(), "plasmid.fasta".to_string(), "virus.fasta".to_string()]), query_id: Some(4), query_name: Some("r16".to_string()) },
//!                                 PseudoAln { ones: Some(vec![]), ones_names: Some(vec![]), query_id: Some(2), query_name: Some("r651903".to_string()) }
//!                                ];
//!
//! let mut iter = data.into_iter(); // Encoder::new expects PseudoAln and doesn't work on &PseudoAln
//! let mut encoder = Encoder::new(&mut iter, &targets, &queries, &name);
//!
//! let mut bytes = encoder.encode_header_and_flags().unwrap();
//! for mut data in encoder.by_ref() {
//!     bytes.append(&mut data);
//! }
//!
//! // Decode from `bytes` to Metagraph plaintext format
//! let mut input = Cursor::new(&bytes);
//! let mut decoder = Decoder::new(&mut input);
//! let mut printer = Printer::new(&mut decoder, &targets, &queries, &name, Format::Metagraph);
//!
//! let mut output: Vec<u8> = Vec::new();
//! for mut line in printer.by_ref() {
//!     output.append(&mut line);
//! }
//!
//! // Expect this plain text output
//! //   0    r1       virus.fasta
//! //   3    r7543    chr.fasta:virus.fasta
//! //   4    r16      chr.fasta:plasmid.fasta:virus.fasta
//! //   2    r651903
//! //
//! let mut expected: Vec<u8> = Vec::new();
//! expected.append(&mut b"0\tr1\tvirus.fasta\n".to_vec());
//! expected.append(&mut b"3\tr7543\tchr.fasta:virus.fasta\n".to_vec());
//! expected.append(&mut b"4\tr16\tchr.fasta:plasmid.fasta:virus.fasta\n".to_vec());
//! expected.append(&mut b"2\tr651903\t\n".to_vec());
//!
//! assert_eq!(output, expected);
//! ```
//!

use crate::Format;
use crate::PseudoAln;
use crate::headers::file::FileHeader;
use crate::headers::file::FileFlags;
use crate::headers::file::build_header_and_flags;

use bifrost::format_bifrost_header;

use bifrost::format_bifrost_line;
use fulgor::format_fulgor_line;
use metagraph::format_metagraph_line;
use sam::build_sam_header;
use sam::format_sam_line;
use sam::format_sam_header;
use themisto::format_themisto_line;

// Format specific implementations
pub mod bifrost;
pub mod fulgor;
pub mod metagraph;
pub mod sam;
pub mod themisto;

pub struct Printer<'a, I: Iterator> where I: Iterator<Item=PseudoAln> {
    // Inputs
    records: &'a mut I,

    header: FileHeader,
    flags: FileFlags,

    sam_header: Option<noodles_sam::Header>,

    index: usize,
    pub format: Format,
}

impl<'a, I: Iterator> Printer<'a, I> where I: Iterator<Item=PseudoAln> {
    pub fn new(
        records: &'a mut I,
        targets: &[String],
        queries: &[String],
        sample_name: &str,
        format: Format,
    ) -> Self {
        let (header, flags) = build_header_and_flags(targets, queries, sample_name).unwrap();
        let sam_header = if format == Format::SAM {
            Some(sam::build_sam_header(&flags.target_names).unwrap())
        } else {
            None
        };

        Printer{
            records,
            header, flags,
            sam_header, index: 0,
            format,
        }
    }

    pub fn new_from_header_and_flags(
        records: &'a mut I,
        header: FileHeader,
        flags: FileFlags,
        format: Format,
    ) -> Self {
        let sam_header = if format == Format::SAM {
            Some(sam::build_sam_header(&flags.target_names).unwrap())
        } else {
            None
        };

        Printer{
            records,
            header, flags,
            sam_header, index: 0,
            format,
        }
    }
}

impl<'a, I: Iterator> Printer<'a, I> where I: Iterator<Item=PseudoAln> {
    pub fn print_header(
        &mut self,
    ) -> Option<Vec<u8>> {
        let mut out: Vec<u8> = Vec::new();
        match self.format {
            Format::Themisto => None,
            Format::Fulgor => None,
            Format::Metagraph => None,
            Format::Bifrost => {
                format_bifrost_header(&self.flags.target_names, &mut out).unwrap();
                Some(out)
            },
            Format::SAM => {
                self.sam_header = Some(build_sam_header(&self.flags.target_names).unwrap());
                format_sam_header(self.sam_header.as_ref().unwrap(), &mut out).unwrap();
                Some(out)
            },
        }
    }
}

impl<'a, I: Iterator> Iterator for Printer<'a, I> where I: Iterator<Item=PseudoAln> {
    type Item = Vec<u8>;

    fn next(
        &mut self,
    ) -> Option<Vec<u8>> {
        let mut out: Vec<u8> = Vec::new();
        if self.index == 0 {
            if let Some(mut header) = self.print_header() {
                out.append(&mut header);
            }
        }

        if let Some(record) = self.records.next() {
            match self.format {
                Format::Themisto => format_themisto_line(&record, &mut out).unwrap(),
                Format::Fulgor => format_fulgor_line(&record, &mut out).unwrap(),
                Format::Metagraph => format_metagraph_line(&record, &mut out).unwrap(),
                Format::Bifrost => format_bifrost_line(&record, self.header.n_targets as usize, &mut out).unwrap(),
                Format::SAM => format_sam_line(&record, self.sam_header.as_ref().unwrap(), &mut out).unwrap(),
            }
            self.index += 1;
            Some(out)
        } else {
            None
        }
    }

}

// Tests
#[cfg(test)]
mod tests {
    #[test]
    fn print_themisto_output() {
        use super::Printer;

        use crate::Format;
        use crate::FileFlags;
        use crate::FileHeader;
        use crate::PseudoAln;

        use std::io::Cursor;
        use std::io::Write;

        let data = vec![
            PseudoAln{ones_names: None,  query_id: Some(128), ones: Some(vec![0, 7, 11, 3]), ..Default::default()},
            PseudoAln{ones_names: None,  query_id: Some(7),   ones: Some(vec![3, 2, 1, 0]), ..Default::default()},
            PseudoAln{ones_names: None,  query_id: Some(8),   ones: Some(vec![]), ..Default::default()},
            PseudoAln{ones_names: None,  query_id: Some(0),   ones: Some(vec![]), ..Default::default()},
            PseudoAln{ones_names: None,  query_id: Some(1),   ones: Some(vec![4, 2, 9, 7]), ..Default::default()},
        ];

        let flags = FileFlags { query_name: "ERR4035126".to_string(), target_names: vec!["chromosome.fasta".to_string(), "plasmid.fasta".to_string()] };
        let header = FileHeader { n_targets: 2, n_queries: 5, flags_len: 0, format: 0, ph2: 0, ph3: 0, ph4: 0 };
        let expected: Vec<u8> = vec![b"128 0 7 11 3\n".to_vec(),
                                     b"7 3 2 1 0\n".to_vec(),
                                     b"8\n".to_vec(),
                                     b"0\n".to_vec(),
                                     b"1 4 2 9 7\n".to_vec(),
        ].concat();

        let mut cursor: Cursor<Vec<u8>> = Cursor::new(Vec::new());

        let mut data_iter = data.into_iter();
        let mut printer = Printer::new_from_header_and_flags(&mut data_iter, header, flags, Format::Themisto);
        for bytes in printer.by_ref() {
            cursor.write_all(&bytes).unwrap();
        }

        let got = cursor.get_ref();

        assert_eq!(got, &expected);
    }

    #[test]
    fn print_fulgor_output() {
        use super::Printer;

        use crate::Format;
        use crate::FileFlags;
        use crate::FileHeader;
        use crate::PseudoAln;

        use std::io::Cursor;
        use std::io::Write;

        let data = vec![
            PseudoAln{ones_names: None,  query_id: None, ones: Some(vec![]), query_name: Some("ERR4035126.4996".to_string()) },
            PseudoAln{ones_names: None,  query_id: None, ones: Some(vec![0]), query_name: Some("ERR4035126.1262953".to_string()) },
            PseudoAln{ones_names: None,  query_id: None, ones: Some(vec![1]), query_name: Some("ERR4035126.1262954".to_string()) },
            PseudoAln{ones_names: None,  query_id: None, ones: Some(vec![1]), query_name: Some("ERR4035126.1262955".to_string()) },
            PseudoAln{ones_names: None,  query_id: None, ones: Some(vec![0]), query_name: Some("ERR4035126.1262956".to_string()) },
            PseudoAln{ones_names: None,  query_id: None, ones: Some(vec![0]), query_name: Some("ERR4035126.1262957".to_string()) },
            PseudoAln{ones_names: None,  query_id: None, ones: Some(vec![0]), query_name: Some("ERR4035126.1262958".to_string()) },
            PseudoAln{ones_names: None,  query_id: None, ones: Some(vec![0]), query_name: Some("ERR4035126.1262959".to_string()) },
            PseudoAln{ones_names: None,  query_id: None, ones: Some(vec![0, 1]), query_name: Some("ERR4035126.651965".to_string()) },
            PseudoAln{ones_names: None,  query_id: None, ones: Some(vec![]), query_name: Some("ERR4035126.11302".to_string()) },
            PseudoAln{ones_names: None,  query_id: None, ones: Some(vec![0]), query_name: Some("ERR4035126.1262960".to_string()) },
            PseudoAln{ones_names: None,  query_id: None, ones: Some(vec![0]), query_name: Some("ERR4035126.1262961".to_string()) },
            PseudoAln{ones_names: None,  query_id: None, ones: Some(vec![0]), query_name: Some("ERR4035126.1262962".to_string()) },
            PseudoAln{ones_names: None,  query_id: None, ones: Some(vec![0, 1]), query_name: Some("ERR4035126.651965".to_string()) },
        ];

        let flags = FileFlags { query_name: "ERR4035126".to_string(), target_names: vec!["chromosome.fasta".to_string(), "plasmid.fasta".to_string()] };
        let header = FileHeader { n_targets: 2, n_queries: 14, flags_len: 0, format: 0, ph2: 0, ph3: 0, ph4: 0 };

        let mut expected: Vec<u8> = b"ERR4035126.4996\t0\n".to_vec();
        expected.append(&mut b"ERR4035126.1262953\t1\t0\n".to_vec());
        expected.append(&mut b"ERR4035126.1262954\t1\t1\n".to_vec());
        expected.append(&mut b"ERR4035126.1262955\t1\t1\n".to_vec());
        expected.append(&mut b"ERR4035126.1262956\t1\t0\n".to_vec());
        expected.append(&mut b"ERR4035126.1262957\t1\t0\n".to_vec());
        expected.append(&mut b"ERR4035126.1262958\t1\t0\n".to_vec());
        expected.append(&mut b"ERR4035126.1262959\t1\t0\n".to_vec());
        expected.append(&mut b"ERR4035126.651965\t2\t0\t1\n".to_vec());
        expected.append(&mut b"ERR4035126.11302\t0\n".to_vec());
        expected.append(&mut b"ERR4035126.1262960\t1\t0\n".to_vec());
        expected.append(&mut b"ERR4035126.1262961\t1\t0\n".to_vec());
        expected.append(&mut b"ERR4035126.1262962\t1\t0\n".to_vec());
        expected.append(&mut b"ERR4035126.651965\t2\t0\t1\n".to_vec());

        let mut cursor: Cursor<Vec<u8>> = Cursor::new(Vec::new());

        let mut data_iter = data.into_iter();
        let mut printer = Printer::new_from_header_and_flags(&mut data_iter, header, flags, Format::Fulgor);
        for bytes in printer.by_ref() {
            cursor.write_all(&bytes).unwrap();
        }

        let got = cursor.get_ref();

        assert_eq!(got, &expected);
    }

    #[test]
    fn print_bifrost_output() {
        use super::Printer;

        use crate::Format;
        use crate::FileFlags;
        use crate::FileHeader;
        use crate::PseudoAln;

        use std::io::Cursor;
        use std::io::Write;

        let data = vec![
            PseudoAln{ query_name: Some("ERR4035126.724962".to_string()), ones: Some(vec![]), ones_names: None, query_id: None },
            PseudoAln{ query_name: Some("ERR4035126.1235744".to_string()), ones: Some(vec![]), ones_names: None, query_id: None },
            PseudoAln{ query_name: Some("ERR4035126.431001".to_string()), ones: Some(vec![]), ones_names: None, query_id: None },
            PseudoAln{ query_name: Some("ERR4035126.645400".to_string()), ones: Some(vec![]), ones_names: None, query_id: None },
            PseudoAln{ query_name: Some("ERR4035126.3001".to_string()), ones: Some(vec![0]), ones_names: None, query_id: None },
            PseudoAln{ query_name: Some("ERR4035126.515778".to_string()), ones: Some(vec![0]), ones_names: None, query_id: None },
            PseudoAln{ query_name: Some("ERR4035126.886205".to_string()), ones: Some(vec![0]), ones_names: None, query_id: None },
            PseudoAln{ query_name: Some("ERR4035126.1254676".to_string()), ones: Some(vec![0]), ones_names: None, query_id: None },
            PseudoAln{ query_name: Some("ERR4035126.668031".to_string()), ones: Some(vec![1]), ones_names: None, query_id: None },
            PseudoAln{ query_name: Some("ERR4035126.388619".to_string()), ones: Some(vec![0]), ones_names: None, query_id: None },
            PseudoAln{ query_name: Some("ERR4035126.959743".to_string()), ones: Some(vec![]), ones_names: None, query_id: None },
            PseudoAln{ query_name: Some("ERR4035126.1146685".to_string()), ones: Some(vec![]), ones_names: None, query_id: None },
            PseudoAln{ query_name: Some("ERR4035126.1017809".to_string()), ones: Some(vec![]), ones_names: None, query_id: None },
            PseudoAln{ query_name: Some("ERR4035126.788136".to_string()), ones: Some(vec![]), ones_names: None, query_id: None },
            PseudoAln{ query_name: Some("ERR4035126.1223924".to_string()), ones: Some(vec![0, 1]), ones_names: None, query_id: None },
            PseudoAln{ query_name: Some("ERR4035126.910807".to_string()), ones: Some(vec![]), ones_names: None, query_id: None },
            PseudoAln{ query_name: Some("ERR4035126.824748".to_string()), ones: Some(vec![0]), ones_names: None, query_id: None },
        ];

        let flags = FileFlags { query_name: "ERR4035126".to_string(), target_names: vec!["chromosome.fasta".to_string(), "plasmid.fasta".to_string()] };
        let header = FileHeader { n_targets: 2, n_queries: 17, flags_len: 0, format: 0, ph2: 0, ph3: 0, ph4: 0 };

        let mut expected: Vec<u8> = b"query_name\tchromosome.fasta\tplasmid.fasta\n".to_vec();
        expected.append(&mut b"ERR4035126.724962\t0\t0\n".to_vec());
        expected.append(&mut b"ERR4035126.1235744\t0\t0\n".to_vec());
        expected.append(&mut b"ERR4035126.431001\t0\t0\n".to_vec());
        expected.append(&mut b"ERR4035126.645400\t0\t0\n".to_vec());
        expected.append(&mut b"ERR4035126.3001\t1\t0\n".to_vec());
        expected.append(&mut b"ERR4035126.515778\t1\t0\n".to_vec());
        expected.append(&mut b"ERR4035126.886205\t1\t0\n".to_vec());
        expected.append(&mut b"ERR4035126.1254676\t1\t0\n".to_vec());
        expected.append(&mut b"ERR4035126.668031\t0\t1\n".to_vec());
        expected.append(&mut b"ERR4035126.388619\t1\t0\n".to_vec());
        expected.append(&mut b"ERR4035126.959743\t0\t0\n".to_vec());
        expected.append(&mut b"ERR4035126.1146685\t0\t0\n".to_vec());
        expected.append(&mut b"ERR4035126.1017809\t0\t0\n".to_vec());
        expected.append(&mut b"ERR4035126.788136\t0\t0\n".to_vec());
        expected.append(&mut b"ERR4035126.1223924\t1\t1\n".to_vec());
        expected.append(&mut b"ERR4035126.910807\t0\t0\n".to_vec());
        expected.append(&mut b"ERR4035126.824748\t1\t0\n".to_vec());

        let mut cursor: Cursor<Vec<u8>> = Cursor::new(Vec::new());

        let mut data_iter = data.into_iter();
        let mut printer = Printer::new_from_header_and_flags(&mut data_iter, header, flags, Format::Bifrost);
        for bytes in printer.by_ref() {
            cursor.write_all(&bytes).unwrap();
        }

        let got = cursor.get_ref();

        assert_eq!(got, &expected);
    }

    #[test]
    fn print_metagraph_output() {
        use super::Printer;

        use crate::Format;
        use crate::FileFlags;
        use crate::FileHeader;
        use crate::PseudoAln;

        use std::io::Cursor;
        use std::io::Write;

        let data = vec![
            PseudoAln{ones_names: Some(vec!["chr.fasta".to_string()]),  query_id: Some(3), ones: Some(vec![]), query_name: Some("ERR4035126.2".to_string()) },
            PseudoAln{ones_names: Some(vec!["chr.fasta".to_string()]),  query_id: Some(2), ones: Some(vec![]), query_name: Some("ERR4035126.1".to_string()) },
            PseudoAln{ones_names: Some(vec!["chr.fasta".to_string(), "plasmid.fasta".to_string()]),  query_id: Some(1303804), ones: Some(vec![]), query_name: Some("ERR4035126.651903".to_string()) },
            PseudoAln{ones_names: Some(vec![]),  query_id: Some(30), ones: Some(vec![]), query_name: Some("ERR4035126.16".to_string()) },
            PseudoAln{ones_names: Some(vec!["plasmid.fasta".to_string()]),  query_id: Some(15084), ones: Some(vec![]), query_name: Some("ERR4035126.7543".to_string()) },
        ];

        let flags = FileFlags { query_name: "ERR4035126".to_string(), target_names: vec!["chromosome.fasta".to_string(), "plasmid.fasta".to_string()] };
        let header = FileHeader { n_targets: 2, n_queries: 5, flags_len: 0, format: 0, ph2: 0, ph3: 0, ph4: 0 };

        let mut expected: Vec<u8> = b"3\tERR4035126.2\tchr.fasta\n".to_vec();
        expected.append(&mut b"2\tERR4035126.1\tchr.fasta\n".to_vec());
        expected.append(&mut b"1303804\tERR4035126.651903\tchr.fasta:plasmid.fasta\n".to_vec());
        expected.append(&mut b"30\tERR4035126.16\t\n".to_vec());
        expected.append(&mut b"15084\tERR4035126.7543\tplasmid.fasta\n".to_vec());

        let mut cursor: Cursor<Vec<u8>> = Cursor::new(Vec::new());

        let mut data_iter = data.into_iter();
        let mut printer = Printer::new_from_header_and_flags(&mut data_iter, header, flags, Format::Metagraph);
        for bytes in printer.by_ref() {
            cursor.write_all(&bytes).unwrap();
        }

        let got = cursor.get_ref();

        assert_eq!(got, &expected);
    }

    // #[test]
    // fn print_sam_output() {
    //     use crate::Format;
    //     use crate::FileFlags;
    //     use crate::FileHeader;
    //     use std::io::Cursor;

    //     use super::cat;
    //     use super::PseudoAln;

    //     let data = vec![
    //         PseudoAln{ query_id: None, query_name: Some("ERR4035126.1".to_string()), ones_names: Some(vec!["OZ038621.1".to_string()]), ones: Some(vec![0]) },
    //         PseudoAln{ query_id: None, query_name: Some("ERR4035126.2".to_string()), ones_names: Some(vec!["OZ038621.1".to_string()]), ones: Some(vec![0]) },
    //         PseudoAln{ query_id: None, query_name: Some("ERR4035126.3".to_string()), ones_names: Some(vec!["OZ038621.1".to_string()]), ones: Some(vec![0]) },
    //         PseudoAln{ query_id: None, query_name: Some("ERR4035126.1261584".to_string()), ones_names: Some(vec!["OZ038622.1".to_string()]), ones: Some(vec![1]) },
    //         PseudoAln{ query_id: None, query_name: Some("ERR4035126.1213410".to_string()), ones_names: Some(vec!["OZ038622.1".to_string()]), ones: Some(vec![1]) },
    //         PseudoAln{ query_id: None, query_name: Some("ERR4035126.1213410".to_string()), ones_names: Some(vec!["OZ038621.1".to_string()]), ones: Some(vec![0]) },
    //         PseudoAln{ query_id: None, query_name: Some("ERR4035126.4".to_string()), ones_names: Some(vec!["OZ038621.1".to_string()]), ones: Some(vec![0]) },
    //         PseudoAln{ query_id: None, query_name: Some("ERR4035126.5".to_string()), ones_names: Some(vec!["OZ038621.1".to_string()]), ones: Some(vec![0]) },
    //         PseudoAln{ query_id: None, query_name: Some("ERR4035126.6".to_string()), ones_names: Some(vec!["OZ038621.1".to_string()]), ones: Some(vec![0]) },
    //         PseudoAln{ query_id: None, query_name: Some("ERR4035126.973529".to_string()), ones_names: Some(vec!["OZ038621.1".to_string()]), ones: Some(vec![0]) },
    //         PseudoAln{ query_id: None, query_name: Some("ERR4035126.973529".to_string()), ones_names: Some(vec!["OZ038621.1".to_string()]), ones: Some(vec![0]) },
    //         PseudoAln{ query_id: None, query_name: Some("ERR4035126.621281".to_string()), ones_names: Some(vec!["OZ038621.1".to_string()]), ones: Some(vec![0]) },
    //         PseudoAln{ query_id: None, query_name: Some("ERR4035126.1178767".to_string()), ones_names: None, ones: Some(vec![]) },
    //         PseudoAln{ query_id: None, query_name: Some("ERR4035126.621281".to_string()), ones_names: Some(vec!["OZ038621.1".to_string()]), ones: Some(vec![0]) },
    //         PseudoAln{ query_id: None, query_name: Some("ERR4035126.621281".to_string()), ones_names: Some(vec!["OZ038621.1".to_string()]), ones: Some(vec![0]) },
    //     ];

    //     let flags = FileFlags { query_name: "ERR4035126".to_string(), target_names: vec!["OZ038621.1".to_string(), "OZ038622.1".to_string()] };
    //     let header = FileHeader { n_targets: 2, n_queries: 15, flags_len: 0, format: 0, ph2: 0, ph3: 0, ph4: 0 };

    //     let mut expected: Vec<u8> = b"@HD\tVN:1.6\n".to_vec();
    //     expected.append(&mut b"@SQ\tSN:OZ038621.1\tLN:1\n".to_vec());
    //     expected.append(&mut b"@SQ\tSN:OZ038622.1\tLN:1\n".to_vec());
    //     expected.append(&mut b"ERR4035126.1\t16\tOZ038621.1\t4541508\t60\t151M\t*\t0\t0\tAGTATTTAGTGACCTAAGTCAATAAAATTTTAATTTACTCACGGCAGGTAACCAGTTCAGAAGCTGCTATCAGACACTCTTTTTTTAATCCACACAGAGACATATTGCCCGTTGCAGTCAGAATGAAAAGCTGAAAATCACTTACTAAGGC\tFJ<<JJFJAA<-JFAJFAF<JFFJJJJJJJFJFJJA<A<AJJAAAFFJJJJFJJFJFJAJJ7JJJJJFJJJJJFFJFFJFJJJJJJFJ7FFJAJJJJJJJJFJJFJJFJFJJJJFJJFJJJJJJJJJFFJJJJJJJJJJJJJFJJJFFAAA\tNM:i:0\tMD:Z:151\tAS:i:151\tXS:i:0\n".to_vec());
    //     expected.append(&mut b"ERR4035126.2\t16\tOZ038621.1\t4541557\t60\t151M\t*\t0\t0\tAACCAGTTCAGAAGCTGCTATCAGACACTCTTTTTTTAATCCACACAGAGACATATTGCCCGTTGCAGTCAGAATGAAAAGCTGAAAATCACTTACTAAGGCGTTTTTTATTTGGTGATATTTTTTTCAATATCATGCAGCAAACGGTGCA\tJAFJFJJJFFJFAJJJJJJJJJJFFA<JJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJFJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJFJJJJJJJJJJJJJJJFF-FFFAA\tNM:i:0\tMD:Z:151\tAS:i:151\tXS:i:0\n".to_vec());
    //     expected.append(&mut b"ERR4035126.3\t16\tOZ038621.1\t4541521\t60\t151M\t*\t0\t0\tCTAAGTCAATAAAATTTTAATTTACTCACGGCAGGTAACCAGTTCAGAAGCTGCTATCAGACACTCTTTTTTTAATCCACACAGAGACATATTGCCCGTTGCAGTCAGAATGAAAAGCTGAAAATCACTTACTAAGGCGTTTTTTATTTGG\tJJJJJJJFJFFFJJJJJJAJJJF7JJJJJ<JJFFJJJJJJJFJJJJJJJJJFFFJJJFJJJJJJJJJJJJJJJJAJFJJJJFJJJJJJJJJJJJJJJJJJJJJJAJJJJJJJJJJJJJJJJJAJFJFJJJJJJJJJJJJJJJJJFJFAFAA\tNM:i:0\tMD:Z:151\tAS:i:151\tXS:i:0\n".to_vec());
    //     expected.append(&mut b"ERR4035126.1261584\t16\tOZ038622.1\t66398\t60\t151M\t*\t0\t0\tGCCGCTGTCTGAACCATGATATTGGCGGAACCGATGCCCATGATGGATGCGCCCCACAGCATGACCAGTTGCGCCAGACTCCAGCCGGAAGCGGTGGGCACAATCATCAAAAATCCACTCACGACACTGAGTATGCCGACGACGTCCCGTC\tFFJJJJFFJFJFFFJJJJJJJJJJJ7FA<JJ<JFJJFJJJJF-FJJA<FJJJJAJJJJJJJJJJJJJJJJJJJJFFJJJFJJJJJJJJJJJJJJJJJJFJFJJJJJJJJJFJJJJJFJJFJFJFJJJJJJJJJJJJJJJJJJJJJJFFFAA\tNM:i:0\tMD:Z:151\tAS:i:151\tXS:i:0\n".to_vec());
    //     expected.append(&mut b"ERR4035126.1213410\t16\tOZ038622.1\t3996\t60\t151M\t*\t0\t0\tGCTGGCGCTTCGGGGATATGTGTTTCGACGGCAGATGAATTTATTCCGGCGGGGGCTGATTCTGCCGTCTGTTCAGTAAATACAGGTGCGATAATATCTGTTTTTTCGGATAAGGACGGTGGCGAAAAAGTACGACGTTTTTTCACCACAA\tJJJJJJJJJJJJJJJJJJJJJJJJJJJFJFJFFJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJFJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJFFJJJJJJJJJJJFFAAA\tNM:i:0\tMD:Z:151\tAS:i:151\tXS:i:0\n".to_vec());
    //     expected.append(&mut b"ERR4035126.1213410\t16\tOZ038621.1\t3996\t60\t151M\t*\t0\t0\tGCTGGCGCTTCGGGGATATGTGTTTCGACGGCAGATGAATTTATTCCGGCGGGGGCTGATTCTGCCGTCTGTTCAGTAAATACAGGTGCGATAATATCTGTTTTTTCGGATAAGGACGGTGGCGAAAAAGTACGACGTTTTTTCACCACAA\tJJJJJJJJJJJJJJJJJJJJJJJJJJJFJFJFFJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJFJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJFFJJJJJJJJJJJFFAAA\tNM:i:0\tMD:Z:151\tAS:i:151\tXS:i:0\n".to_vec());
    //     expected.append(&mut b"ERR4035126.4\t0\tOZ038621.1\t4541351\t60\t151M\t*\t0\t0\tAGGTGCGGGCTTTTTTCTGTGTTTCCTGTACGCGTCAGCCCGCACCGTTACCTGTGGTAATGGTGATGGTGGTGGTAATGGTGGTGCTAATGCGTTTCATGGATGTTGTGTACTCTGTAATTTTTATCTGTCTGTGCGCTATGCCTATATT\tAAFFFJJJJJJJJJJJJJJJJJJJJJFFJJJJJJJJJJJJJJJJJJJFFJJJJJJJJFJJJJJJJJJJJ<JFJJJJJJJJJJJAJJJFJJJJJJJJJJJJJJJJJJJFJJJJJJJJJJJJJJJJJJJJJJJFFFJFAFJJJJF<FFFJJJJ\tNM:i:0\tMD:Z:151\tAS:i:151\tXS:i:0\n".to_vec());
    //     expected.append(&mut b"ERR4035126.5\t16\tOZ038621.1\t4541533\t60\t151M\t*\t0\t0\tAATTTTAATTTACTCACGGCAGGTAACCAGTTCAGAAGCTGCTATCAGACACTCTTTTTTTAATCCACACAGAGACATATTGCCCGTTGCAGTCAGAATGAAAAGCTGAAAATCACTTACTAAGGCGTTTTTTATTTGGTGATATTTTTTT\tFJJJJJJJJFJJJJJJJJFJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJFJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJFJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJFJJJJJJJJJJJJJFFFAA\tNM:i:0\tMD:Z:151\tAS:i:151\tXS:i:0\n".to_vec());
    //     expected.append(&mut b"ERR4035126.6\t0\tOZ038621.1\t4541261\t60\t151M\t*\t0\t0\tTCTGCATTTGCCACTGATGTACCGCCGAACTTCAACACTCGCATGGTTGTTACCTCGTTACCTTTGGTCGAAAAAAAAGCCCGCACTGTCAGGTGCGGGCTTTTTTCTGTGTTTCCTGTACGCGTCAGCCCGCACCGTTACCTGTGGTAAT\tAAAFFJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJFJJFJJJJJJJJ<FJJJJJJJJJJJJJJJJJJJJJJJJJJJFJJJJJJJJFJJJJF7FJJJJJJJFJFJJJJJJJJFJJJJJJJJAJJJJFJFFFJFJF\tNM:i:0\tMD:Z:151\tAS:i:151\tXS:i:0\n".to_vec());
    //     expected.append(&mut b"ERR4035126.973529\t16\tOZ038621.1\t3695316\t60\t66S85M\t*\t0\t0\tGGAGATGATTTCGTGTTTCTTCTCCGGGATGACCATGTCATCGATACCAACAGATGCACCAGAACGCGCCAAGTCGGGCAATCTGGTGAACTGGAAAGCCGGGGCGCTGTATCACCTGACGGAAAACGGCAATGTCTATATTAACTATGCC\tJJFJFF7-FFJJJA-FJFFFJJFAJJJJJJJJJJJJJJJJFJJJJJJJJFJJJJFJ<JJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJFFJFJJJJJJJJFJJJJJJJJFAJJJJJJJJJJJJJFJJJJJFFJJJFJJJJJJFJJFFFAA\tNM:i:0\tMD:Z:85\tAS:i:85\tXS:i:0\tSA:Z:OZ038621.1,5194124,-,69M82S,60,0;\n".to_vec());
    //     expected.append(&mut b"ERR4035126.973529\t2064\tOZ038621.1\t5194124\t60\t69M82H\t*\t0\t0\tGGAGATGATTTCGTGTTTCTTCTCCGGGATGACCATGTCATCGATACCAACAGATGCACCAGAACGCGC\tJJFJFF7-FFJJJA-FJFFFJJFAJJJJJJJJJJJJJJJJFJJJJJJJJFJJJJFJ<JJJJJJJJJJJJ\tNM:i:0\tMD:Z:69\tAS:i:69\tXS:i:0\tSA:Z:OZ038621.1,3695316,-,66S85M,60,0;\n".to_vec());
    //     expected.append(&mut b"ERR4035126.621281\t16\tOZ038621.1\t1040569\t60\t39S86M26S\t*\t0\t0\tGCTCGACCGCGTCCCAGTTGAAATGCAACTCCCCAGCCAACTCGATAAACACGATGATTAACACGGCAGTCATGGTCAGAATGGAAACGGGATCGAAAATCGGCATACCAAATGACATCGGCGTGCCACAGCACAAACTGGACGCCCTGGC\tAFAJJJJJJJJJJJJFJJJJJJJJJJJJJJJJFJFJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJFFFAA\tNM:i:0\tMD:Z:86\tAS:i:86\tXS:i:0\tSA:Z:OZ038621.1,3172373,-,46M105S,60,0;OZ038621.1,1301509,+,33M118S,60,0;\n".to_vec());
    //     expected.append(&mut b"ERR4035126.1178767\t4\t*\t0\t0\t*\t*\t0\t0\tACTTGGCTCATGTTCCGTCAATGCCGGAGAGACAATTGAAGTTGATTTAGGTGATGTCTTCGCTGCCAATTTCCGTGTTGTAGGGCATAAACCTCTTGGGGCCAGAACGGCAGAACTTGCAATTCCAGTCAGGTGTAACACGGGAAACGCG\tAAFFFJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJFJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJFJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJJ\tAS:i:0\tXS:i:0\n".to_vec());
    //     expected.append(&mut b"ERR4035126.621281\t2064\tOZ038621.1\t3172373\t60\t46M105H\t*\t0\t0\tGCTCGACCGCGTCCCAGTTGAAATGCAACTCCCCAGCCAACTCGAT\tAFAJJJJJJJJJJJJFJJJJJJJJJJJJJJJJFJFJJJJJJJJJJJ\tNM:i:0\tMD:Z:46\tAS:i:46\tXS:i:0\tSA:Z:OZ038621.1,1040569,-,39S86M26S,60,0;OZ038621.1,1301509,+,33M118S,60,0;\n".to_vec());
    //     expected.append(&mut b"ERR4035126.621281\t2048\tOZ038621.1\t1301509\t60\t33M118H\t*\t0\t0\tGCCAGGGCGTCCAGTTTGTGCTGTGGCACGCCG\tAAFFFJJJJJJJJJJJJJJJJJJJJJJJJJJJJ\tNM:i:0\tMD:Z:33\tAS:i:33\tXS:i:0\tSA:Z:OZ038621.1,1040569,-,39S86M26S,60,0;OZ038621.1,3172373,-,46M105S,60,0;\n".to_vec());

    //     let mut cursor: Cursor<Vec<u8>> = Cursor::new(Vec::new());

    //     cat(&data, &flags, &header, &Format::SAM, &mut cursor).unwrap();
    //     let got = cursor.get_ref();

    //     eprintln!("{}", got.iter().map(|x| *x as char).collect::<String>());
    //     eprintln!("{}", expected.iter().map(|x| *x as char).collect::<String>());
    //     assert_eq!(got, &expected);
    // }
}
