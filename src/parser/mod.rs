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

#[derive(Debug)]
pub struct Parser<'a, R: Read> {
    // conn: R,
    reader: BufReader<&'a mut R>,
    format: Format,
}

impl<'a, R: Read> Parser<'a, R> {
    pub fn new(
        conn: &'a mut R,
    ) -> Self {
        // TODO try to infer the format
        Self { reader: BufReader::new(conn), format: Format::default() }
    }

    pub fn new_with_format(
        conn: &'a mut R,
        format: &Format,
    ) -> Self {
        Self { reader: BufReader::new(conn), format: format.clone() }
    }
}

impl<R: Read> Parser<'_, R> {
    pub fn next(
        &mut self,
    ) -> Option<PseudoAln> {
        let mut line = Cursor::new(Vec::<u8>::new());
        if self.reader.read_until(b'\n', line.get_mut()).is_ok() {
            if line.get_mut().is_empty() {
                return None
            }
            line.get_mut().pop();
            let res = match self.format {
                Format::Themisto{ n_targets: num_targets } => read_themisto(num_targets, &mut line).unwrap(),
                Format::Fulgor{ n_targets: num_targets } => read_fulgor(num_targets, &mut line).unwrap(),
                Format::Bifrost => read_bifrost(&mut line).unwrap(),
            };
            Some(res)
        } else {
            None
        }
    }

    pub fn set_format(
        &mut self,
        format: &Format,
    ) {
        self.format = format.clone();
    }
}

// impl<'a, R: Read> Iterator for &'a Parser<'a, R> {
//     type Item = PseudoAln;

//     fn next(
//         &mut self,
//     ) -> Option<PseudoAln> {
//         let mut line = Cursor::new(Vec::<u8>::new());
//         if self.reader.read_until(b'\n', line.get_mut()).is_ok() {
//             let res = match self.format {
//                 Format::Themisto{ n_targets: num_targets } => read_themisto(num_targets, &mut line).unwrap(),
//                 Format::Fulgor{ n_targets: num_targets } => read_fulgor(num_targets, &mut line).unwrap(),
//                 Format::Bifrost => read_bifrost(&mut line).unwrap(),
//             };
//             Some(res)
//         } else {
//             None
//         }
//     }

// }
