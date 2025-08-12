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

use themisto::format_themisto_line;
use fulgor::format_fulgor_line;

// Format specific implementations
pub mod themisto;
pub mod fulgor;

// TODO need to handle target and query names for Bifrost and Metagraph

#[derive(Debug)]
pub struct Printer<'a> {
    records: &'a Vec<PseudoAln>,
    index: usize,
    pub format: Format,
}

impl<'a> Printer<'a> {
    pub fn new(
        records: &'a Vec<PseudoAln>,
    ) -> Self {
        Printer{ records, index: 0, format: Format::default() }
    }

    pub fn new_with_format(
        records: &'a Vec<PseudoAln>,
        format: &Format,
    ) -> Self {
        Printer{ records, index: 0, format: format.clone() }
    }
}

// TODO implement Iterator and IntoIterator

impl Printer<'_> {
    pub fn next(
        &mut self,
    ) -> Option<Vec<u8>> {
        if self.index < self.records.len() {
            let mut out: Vec<u8> = Vec::new();
            match self.format {
                Format::Themisto => format_themisto_line(&self.records[self.index], &mut out).unwrap(),
                Format::Fulgor => format_fulgor_line(&self.records[self.index], &mut out).unwrap(),
                Format::Bifrost => todo!("Bifrost printing is not implemented."),
                Format::SAM => todo!("SAM printing is not implemented."),
            }
            self.index += 1;
            Some(out)
        } else {
            None
        }
    }
}
