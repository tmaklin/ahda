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
use crate::Format;
use crate::PseudoAln;
use crate::headers::file::FileHeader;
use crate::headers::file::FileFlags;

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

pub struct Printer<'a, I: Iterator> where I: Iterator<Item=&'a PseudoAln> {
    // Inputs
    records: &'a mut I,

    header: Option<FileHeader>,
    flags: Option<FileFlags>,

    sam_header: Option<noodles_sam::Header>,

    index: usize,
    pub format: Format,
}

impl<'a, I: Iterator> Printer<'a, I> where I: Iterator<Item=&'a PseudoAln> {
    pub fn new(
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
            header: Some(header), flags: Some(flags),
            sam_header, index: 0,
            format: Format::default()
        }
    }
}

impl<'a, I: Iterator> Printer<'a, I> where I: Iterator<Item=&'a PseudoAln> {
    pub fn print_header(
        &mut self,
    ) -> Option<Vec<u8>> {
        let mut out: Vec<u8> = Vec::new();
        match self.format {
            Format::Themisto => None,
            Format::Fulgor => None,
            Format::Metagraph => None,
            Format::Bifrost => {
                format_bifrost_header(&self.flags.as_ref().unwrap().target_names, &mut out).unwrap();
                Some(out)
            },
            Format::SAM => {
                self.sam_header = Some(build_sam_header(&self.flags.as_ref().unwrap().target_names).unwrap());
                format_sam_header(self.sam_header.as_ref().unwrap(), &mut out).unwrap();
                Some(out)
            },
        }
    }
}

impl<'a, I: Iterator> Iterator for Printer<'a, I> where I: Iterator<Item=&'a PseudoAln> {
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
                Format::Bifrost => format_bifrost_line(&record, self.header.as_ref().unwrap().n_targets as usize, &mut out).unwrap(),
                Format::SAM => format_sam_line(&record, self.sam_header.as_ref().unwrap(), &mut out).unwrap(),
            }
            self.index += 1;
            Some(out)
        } else {
            None
        }
    }

}
