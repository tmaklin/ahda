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
    buf: Cursor<Vec<u8>>,
    pub format: Format,
}

impl<'a, R: Read> Parser<'a, R> {
    pub fn new(
        conn: &'a mut R,
    ) -> Self {
        let mut reader = BufReader::new(conn);
        let mut buf = Cursor::new(Vec::<u8>::new());

        reader.read_until(b'\n', buf.get_mut()).unwrap();

        let format = guess_format(buf.get_ref()).unwrap();

        Self { reader, buf, format }
    }

    pub fn new_with_format(
        conn: &'a mut R,
        format: &Format,
    ) -> Self {
        Self { reader: BufReader::new(conn), format: format.clone(), buf: Cursor::new(Vec::new()) }
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
                Format::Bifrost => read_bifrost(&mut line).unwrap(),
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

    pub fn set_format(
        &mut self,
        format: &Format,
    ) {
        self.format = format.clone();
    }
}

pub fn guess_format(
    bytes: &[u8],
) -> Option<Format> {
    let not_themisto: bool = bytes.contains(&b'\t');
    if !not_themisto {
        return Some(Format::Themisto)
    }

    let line = bytes.iter().map(|x| *x as char).collect::<String>();
    let mut records = line.split('\t');

    let bifrost: bool = records.next()? == "query_name";
    if bifrost {
        return Some(Format::Bifrost)
    }

    let mut next = records.next()?;
    if records.next().is_none() {
        next = &next[0..(next.len() - 1)];
    }

    let fulgor: bool = next.parse::<u32>().is_ok();
    if fulgor {
        return Some(Format::Fulgor)
    }

    None
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
