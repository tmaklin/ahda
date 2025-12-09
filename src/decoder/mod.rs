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

pub mod bitmap;
pub mod unpack_roaring;

use crate::PseudoAln;
use crate::headers::file::FileHeader;
use crate::headers::file::FileFlags;
use crate::headers::file::read_file_header;
use crate::headers::file::read_file_flags;
use crate::headers::block::BlockHeader;
use crate::headers::block::BlockFlags;
use crate::headers::block::read_block_header;
use unpack_roaring::unpack_block_roaring;

use std::io::Read;

pub struct Decoder<'a, R: Read> {
    // Inputs
    conn: &'a mut R,

    header: FileHeader,
    flags: FileFlags,

    // Internals
    block_header: Option<BlockHeader>,
    block_flags: Option<BlockFlags>,
}

impl<'a, R: Read> Decoder<'a, R> {
    pub fn new(
        conn: &'a mut R,
    ) -> Self {

        let header = read_file_header(conn).unwrap();
        let flags = read_file_flags(&header, conn).unwrap();

        Decoder{
            conn,
            header, flags,
            block_header: None, block_flags: None,
        }
    }
}

impl<R: Read> Decoder<'_, R> {

    pub fn file_header(
        &self,
    ) -> &FileHeader {
        &self.header
    }

    pub fn file_flags(
        &self,
    ) -> &FileFlags {
        &self.flags
    }
}

// TODO This should return a single pseudoalignment using BitmapDecoder
impl<R: Read> Iterator for Decoder<'_, R> {
    type Item = Vec<PseudoAln>;

    fn next(
        &mut self,
    ) -> Option<Self::Item> {

        match read_block_header(self.conn) {
            Ok(block_header) => {
                let mut bytes: Vec<u8> = vec![0; block_header.deflated_len as usize];
                self.conn.read_exact(&mut bytes).unwrap();
                let (bitmap, block_flags) = unpack_block_roaring(&bytes, &block_header).unwrap();

                let mut tmp = bitmap.iter();
                let bitmap_decoder = bitmap::BitmapDecoder::new(&mut tmp, self.header.clone(), self.flags.clone(), block_header.clone(), block_flags.clone());
                let mut alns: Vec<PseudoAln> = Vec::new();
                for record in bitmap_decoder {
                    alns.push(record);
                }

                self.block_header = Some(block_header);
                self.block_flags = Some(block_flags);

                Some(alns)
            },
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn file_header_and_file_flags() {
        use super::Decoder;
        use crate::headers::file::FileFlags;
        use crate::headers::file::FileHeader;

        use std::io::Cursor;

        let expected_header = FileHeader { n_targets: 2, n_queries: 5, flags_len: 36, format: 1, ph2: 0, ph3: 0, ph4: 0 };
        let expected_flags = FileFlags { query_name: "ERR4035126".to_string(), target_names: vec!["chr.fasta".to_string(), "plasmid.fasta".to_string()] };

        let data_bytes: Vec<u8> = vec![2, 0, 0, 0, 5, 0, 0, 0, 36, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 10, 69, 82, 82, 52, 48, 51, 53, 49, 50, 54, 2, 9, 99, 104, 114, 46, 102, 97, 115, 116, 97, 13, 112, 108, 97, 115, 109, 105, 100, 46, 102, 97, 115, 116, 97, 5, 0, 0, 0, 102, 0, 0, 0, 26, 0, 0, 0, 81, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 147, 239, 230, 96, 0, 131, 255, 155, 141, 18, 18, 18, 82, 24, 24, 197, 216, 24, 13, 206, 30, 57, 112, 232, 192, 169, 3, 39, 15, 156, 122, 44, 37, 146, 146, 148, 144, 147, 149, 145, 178, 44, 189, 229, 140, 161, 136, 203, 163, 25, 51, 165, 162, 164, 36, 62, 43, 121, 207, 254, 168, 252, 241, 140, 175, 111, 79, 164, 164, 228, 140, 136, 25, 140, 102, 251, 13, 119, 102, 51, 48, 48, 0, 0, 158, 168, 250, 0, 82, 0, 0, 0];
        let mut data: Cursor<Vec<u8>> = Cursor::new(data_bytes);

        let decoder = Decoder::new(&mut data);

        let got_header = decoder.file_header().clone();
        let got_flags = decoder.file_flags().clone();

        assert_eq!(got_header, expected_header);
        assert_eq!(got_flags, expected_flags);
    }

    #[test]
    fn next() {
        use super::Decoder;
        use crate::PseudoAln;

        use std::io::Cursor;

        let mut expected = vec![
            PseudoAln{ones_names: Some(vec!["chr.fasta".to_string()]),  query_id: Some(1), ones: Some(vec![0]), query_name: Some("ERR4035126.2".to_string()) },
            PseudoAln{ones_names: Some(vec!["chr.fasta".to_string()]),  query_id: Some(0), ones: Some(vec![0]), query_name: Some("ERR4035126.1".to_string()) },
            PseudoAln{ones_names: Some(vec!["chr.fasta".to_string(), "plasmid.fasta".to_string()]),  query_id: Some(2), ones: Some(vec![0, 1]), query_name: Some("ERR4035126.651903".to_string()) },
            PseudoAln{ones_names: Some(vec![]),  query_id: Some(4), ones: Some(vec![]), query_name: Some("ERR4035126.16".to_string()) },
            PseudoAln{ones_names: Some(vec!["plasmid.fasta".to_string()]),  query_id: Some(3), ones: Some(vec![1]), query_name: Some("ERR4035126.7543".to_string()) },
        ];
        expected.sort_by_key(|x| *x.query_id.as_ref().unwrap());

        let data_bytes: Vec<u8> = vec![2, 0, 0, 0, 5, 0, 0, 0, 36, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 10, 69, 82, 82, 52, 48, 51, 53, 49, 50, 54, 2, 9, 99, 104, 114, 46, 102, 97, 115, 116, 97, 13, 112, 108, 97, 115, 109, 105, 100, 46, 102, 97, 115, 116, 97, 5, 0, 0, 0, 103, 0, 0, 0, 26, 0, 0, 0, 81, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 147, 239, 230, 96, 0, 131, 255, 155, 141, 18, 18, 18, 82, 24, 24, 197, 216, 24, 13, 206, 30, 57, 112, 232, 192, 169, 3, 231, 14, 156, 122, 44, 37, 146, 146, 148, 144, 147, 149, 145, 178, 44, 189, 227, 140, 161, 144, 203, 163, 25, 51, 165, 162, 164, 36, 62, 43, 119, 206, 152, 61, 75, 226, 179, 210, 107, 211, 228, 212, 132, 148, 164, 52, 70, 134, 146, 247, 91, 214, 102, 51, 48, 48, 0, 0, 206, 10, 209, 169, 83, 0, 0, 0];
        let mut data: Cursor<Vec<u8>> = Cursor::new(data_bytes);

        let mut decoder = Decoder::new(&mut data);

        let mut got = decoder.next().unwrap();
        got.sort_by_key(|x| *x.query_id.as_ref().unwrap());

        assert_eq!(got, expected);
    }

    #[test]
    fn decode_three_blocks() {
        use super::Decoder;
        use crate::PseudoAln;

        use std::io::Cursor;

        let mut expected = vec![
            PseudoAln{ones_names: Some(vec!["chr.fasta".to_string()]),  query_id: Some(1), ones: Some(vec![0]), query_name: Some("ERR4035126.2".to_string()) },
            PseudoAln{ones_names: Some(vec!["chr.fasta".to_string()]),  query_id: Some(0), ones: Some(vec![0]), query_name: Some("ERR4035126.1".to_string()) },
            PseudoAln{ones_names: Some(vec!["chr.fasta".to_string(), "plasmid.fasta".to_string()]),  query_id: Some(2), ones: Some(vec![0, 1]), query_name: Some("ERR4035126.651903".to_string()) },
            PseudoAln{ones_names: Some(vec![]),  query_id: Some(4), ones: Some(vec![]), query_name: Some("ERR4035126.16".to_string()) },
            PseudoAln{ones_names: Some(vec!["plasmid.fasta".to_string()]),  query_id: Some(3), ones: Some(vec![1]), query_name: Some("ERR4035126.7543".to_string()) },
        ];
        expected.sort_by_key(|x| *x.query_id.as_ref().unwrap());

        let data_bytes: Vec<u8> = vec![2, 0, 0, 0, 5, 0, 0, 0, 36, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 10, 69, 82, 82, 52, 48, 51, 53, 49, 50, 54, 2, 9, 99, 104, 114, 46, 102, 97, 115, 116, 97, 13, 112, 108, 97, 115, 109, 105, 100, 46, 102, 97, 115, 116, 97, 2, 0, 0, 0, 72, 0, 0, 0, 20, 0, 0, 0, 30, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 147, 239, 230, 96, 0, 131, 255, 155, 141, 18, 18, 18, 82, 24, 24, 221, 216, 24, 13, 206, 30, 57, 112, 228, 177, 148, 72, 74, 82, 66, 78, 86, 70, 202, 178, 244, 142, 51, 134, 73, 73, 9, 44, 12, 166, 66, 39, 86, 27, 49, 48, 48, 0, 0, 86, 244, 9, 212, 54, 0, 0, 0, 2, 0, 0, 0, 76, 0, 0, 0, 20, 0, 0, 0, 36, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 147, 239, 230, 96, 0, 131, 255, 155, 141, 18, 18, 18, 82, 24, 24, 221, 216, 24, 53, 206, 170, 188, 144, 18, 73, 73, 74, 200, 201, 202, 72, 89, 150, 158, 150, 149, 145, 153, 112, 166, 91, 37, 41, 227, 216, 145, 35, 76, 12, 159, 242, 186, 250, 45, 24, 24, 24, 0, 233, 70, 193, 6, 59, 0, 0, 0, 1, 0, 0, 0, 69, 0, 0, 0, 18, 0, 0, 0, 19, 0, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 147, 239, 230, 96, 0, 131, 255, 155, 141, 18, 18, 18, 82, 26, 24, 24, 217, 4, 102, 79, 188, 111, 170, 113, 226, 200, 129, 27, 87, 46, 156, 240, 57, 127, 231, 202, 145, 25, 19, 37, 25, 122, 190, 105, 11, 170, 50, 48, 48, 0, 0, 42, 148, 23, 51, 50, 0, 0, 0];
        let mut data: Cursor<Vec<u8>> = Cursor::new(data_bytes);

        let mut decoder = Decoder::new(&mut data);

        let mut got: Vec<PseudoAln> = Vec::new();
        for block in decoder.by_ref() {
            got.append(&mut block.clone());
        }
        got.sort_by_key(|x| *x.query_id.as_ref().unwrap());

        assert_eq!(got, expected);
        assert_eq!(0, 1);
    }
}
