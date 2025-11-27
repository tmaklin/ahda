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

// TODO this should just store FileFlags/FileHeader
#[derive(Debug)]
pub struct Printer<'a> {
    records: &'a [PseudoAln],
    targets: Option<Vec<String>>,
    sam_header: Option<noodles_sam::Header>,
    index: usize,
    pub format: Format,
}

impl<'a> Printer<'a> {
    pub fn new(
        records: &'a Vec<PseudoAln>,
    ) -> Self {
        Printer{ sam_header: None, records, targets: None, index: 0, format: Format::default() }
    }

    pub fn new_from_flags(
        records: &'a [PseudoAln],
        flags: &FileFlags,
        format: &Format,
    ) -> Self {
        let sam_header = if *format == Format::SAM {
            Some(sam::build_sam_header(&flags.target_names).unwrap())
        } else {
            None
        };
        Printer{ sam_header, records, targets: Some(flags.target_names.clone()), format: format.clone(), index: 0 }
    }

    pub fn new_with_format(
        records: &'a [PseudoAln],
        format: &Format,
    ) -> Self {
        Printer{ sam_header: None, records, format: format.clone(), targets: None, index: 0 }
    }
}

impl Printer<'_> {
    pub fn print_header(
        &mut self,
    ) -> Option<Vec<u8>> {
        let mut out: Vec<u8> = Vec::new();
        match self.format {
            Format::Themisto => None,
            Format::Fulgor => None,
            Format::Metagraph => None,
            Format::Bifrost => {
                format_bifrost_header(self.targets.as_ref().unwrap(), &mut out).unwrap();
                Some(out)
            },
            Format::SAM => {
                self.sam_header = Some(build_sam_header(self.targets.as_ref().unwrap()).unwrap());
                format_sam_header(self.sam_header.as_ref().unwrap(), &mut out).unwrap();
                Some(out)
            },
        }
    }
}

impl Iterator for Printer<'_> {
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

        if self.index < self.records.len() {
            match self.format {
                Format::Themisto => format_themisto_line(&self.records[self.index], &mut out).unwrap(),
                Format::Fulgor => format_fulgor_line(&self.records[self.index], &mut out).unwrap(),
                Format::Metagraph => format_metagraph_line(&self.records[self.index], &mut out).unwrap(),
                Format::Bifrost => format_bifrost_line(&self.records[self.index], self.targets.as_ref().unwrap().len(), &mut out).unwrap(),
                Format::SAM => format_sam_line(&self.records[self.index], self.sam_header.as_ref().unwrap(), &mut out).unwrap(),
            }
            self.index += 1;
            Some(out)
        } else {
            None
        }
    }

}
