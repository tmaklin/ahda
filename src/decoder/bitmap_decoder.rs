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
use crate::PseudoAln;
use crate::headers::file::FileHeader;

pub struct BitmapDecoder<'a, I: Iterator> where I: Iterator<Item=u64> {
    // Inputs
    bits_iter: &'a mut I,
    index: Option<u64>,

    file_header: FileHeader,
}

impl<'a, I: Iterator> BitmapDecoder<'a, I> where I: Iterator<Item=u64> {
    pub fn new(
        bits_iter: &'a mut I,
        file_header: FileHeader,
    ) -> Self {

        BitmapDecoder {
            bits_iter,
            file_header,
            index: None,
        }
    }
}

impl<I: Iterator> Iterator for BitmapDecoder<'_, I> where I: Iterator<Item=u64>{
    type Item = PseudoAln;

    fn next(
        &mut self,
    ) -> Option<Self::Item> {
        let mut ones: Vec<u32> = Vec::new();
        let mut query_id: Option<u32> = None;

        let n_targets: u64 = self.file_header.n_targets as u64;
        if self.index.is_some() {
            let query_idx = self.index.as_ref().unwrap() / n_targets;
            let target_idx = self.index.as_ref().unwrap() % n_targets;
            ones.push(target_idx as u32);
            query_id = Some(query_idx as u32);
            self.index = None;
        }

        for idx in self.bits_iter.by_ref() {
            self.index = Some(idx);
            let query_idx = self.index.as_ref().unwrap() / n_targets;
            if query_id.is_some() && query_idx as u32 != *query_id.as_ref().unwrap() {
                break;
            }
            let target_idx = self.index.as_ref().unwrap() % n_targets;
            self.index = None;
            ones.push(target_idx as u32);
            query_id = Some(query_idx as u32);
        }

        if query_id.is_some() {
            Some(PseudoAln{
                ones: Some(ones),
                query_id,
                // Filling names for the whole block is slow and takes a lot of space if the alignment is dense
                ones_names: None,
                query_name: None,
            })
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn next_ends_with_one() {
        use super::BitmapDecoder;
        use crate::PseudoAln;
        use crate::compression::MetadataCompression;
        use crate::headers::file::build_file_header_and_flags;

        use roaring::RoaringBitmap;

        let mut expected = vec![
            PseudoAln{ones_names: None,  query_id: Some(1), ones: Some(vec![0]), query_name: None },
            PseudoAln{ones_names: None,  query_id: Some(0), ones: Some(vec![0]), query_name: None },
            PseudoAln{ones_names: None,  query_id: Some(2), ones: Some(vec![0, 1]), query_name: None },
            PseudoAln{ones_names: None,  query_id: Some(3), ones: Some(vec![1]), query_name: None },
        ];
        expected.sort_by_key(|x| *x.query_id.as_ref().unwrap());

        let mut data = RoaringBitmap::new();
        data.insert(0);
        data.insert(2);
        data.insert(4);
        data.insert(5);
        data.insert(7);

        let targets = vec!["chr.fasta".as_bytes().to_vec(), "plasmid.fasta".as_bytes().to_vec()];
        let queries = vec!["ERR4035126.1".as_bytes().to_vec(), "ERR4035126.2".as_bytes().to_vec(), "ERR4035126.651903".as_bytes().to_vec(), "ERR4035126.7543".as_bytes().to_vec()];
        let n_queries = queries.len();
        let (header, _) = build_file_header_and_flags(&targets, n_queries, &"ERR4035126".as_bytes().to_vec(), &MetadataCompression::default()).unwrap();

        let mut tmp = data.iter().map(|x| x as u64);
        let mut bdecoder = BitmapDecoder::new(&mut tmp, header);

        let mut got: Vec<PseudoAln> = Vec::with_capacity(expected.len());
        for record in bdecoder.by_ref() {
            got.push(record);
        }
        got.sort_by_key(|x| *x.query_id.as_ref().unwrap());

        assert_eq!(got, expected);
    }

    #[test]
    fn next_ends_with_ones() {
        use super::BitmapDecoder;
        use crate::PseudoAln;
        use crate::compression::MetadataCompression;
        use crate::headers::file::build_file_header_and_flags;

        use roaring::RoaringBitmap;

        let mut expected = vec![
            PseudoAln{ones_names: None,  query_id: Some(1), ones: Some(vec![0]), query_name: None },
            PseudoAln{ones_names: None,  query_id: Some(0), ones: Some(vec![0]), query_name: None },
            PseudoAln{ones_names: None,  query_id: Some(2), ones: Some(vec![0, 1]), query_name: None },
        ];
        expected.sort_by_key(|x| *x.query_id.as_ref().unwrap());

        let mut data = RoaringBitmap::new();
        data.insert(0);
        data.insert(2);
        data.insert(4);
        data.insert(5);

        let targets = vec!["chr.fasta".as_bytes().to_vec(), "plasmid.fasta".as_bytes().to_vec()];
        let queries = vec!["ERR4035126.1".as_bytes().to_vec(), "ERR4035126.2".as_bytes().to_vec(), "ERR4035126.651903".as_bytes().to_vec(), "ERR4035126.7543".as_bytes().to_vec(), "ERR4035126.16".as_bytes().to_vec()];
        let n_queries = queries.len();
        let (header, _) = build_file_header_and_flags(&targets, n_queries, &"ERR4035126".as_bytes().to_vec(), &MetadataCompression::default()).unwrap();

        let mut tmp = data.iter().map(|x| x as u64);
        let mut bdecoder = BitmapDecoder::new(&mut tmp, header);

        let mut got: Vec<PseudoAln> = Vec::with_capacity(expected.len());
        for record in bdecoder.by_ref() {
            got.push(record);
        }
        got.sort_by_key(|x| *x.query_id.as_ref().unwrap());

        assert_eq!(got, expected);
    }

    #[test]
    fn next_ends_with_zero() {
        use super::BitmapDecoder;
        use crate::PseudoAln;
        use crate::compression::MetadataCompression;
        use crate::headers::file::build_file_header_and_flags;

        use roaring::RoaringBitmap;

        let mut expected = vec![
            PseudoAln{ones_names: None,  query_id: Some(1), ones: Some(vec![0]), query_name: None },
            PseudoAln{ones_names: None,  query_id: Some(0), ones: Some(vec![0]), query_name: None },
            PseudoAln{ones_names: None,  query_id: Some(2), ones: Some(vec![0]), query_name: None },
        ];
        expected.sort_by_key(|x| *x.query_id.as_ref().unwrap());

        let mut data = RoaringBitmap::new();
        data.insert(0);
        data.insert(2);
        data.insert(4);

        let targets = vec!["chr.fasta".as_bytes().to_vec(), "plasmid.fasta".as_bytes().to_vec()];
        let queries = vec!["ERR4035126.1".as_bytes().to_vec(), "ERR4035126.2".as_bytes().to_vec(), "ERR4035126.651903".as_bytes().to_vec(), "ERR4035126.7543".as_bytes().to_vec(), "ERR4035126.16".as_bytes().to_vec()];
        let n_queries = queries.len();
        let (header, _) = build_file_header_and_flags(&targets, n_queries, &"ERR4035126".as_bytes().to_vec(), &MetadataCompression::default()).unwrap();

        let mut tmp = data.iter().map(|x| x as u64);
        let mut bdecoder = BitmapDecoder::new(&mut tmp, header);

        let mut got: Vec<PseudoAln> = Vec::with_capacity(expected.len());
        for record in bdecoder.by_ref() {
            got.push(record);
        }
        got.sort_by_key(|x| *x.query_id.as_ref().unwrap());

        assert_eq!(got, expected);
    }


    #[test]
    fn next_skips_middle() {
        use super::BitmapDecoder;
        use crate::PseudoAln;
        use crate::compression::MetadataCompression;
        use crate::headers::file::build_file_header_and_flags;

        use roaring::RoaringBitmap;

        let mut expected = vec![
            PseudoAln{ones_names: None,  query_id: Some(0), ones: Some(vec![0]), query_name: None },
            PseudoAln{ones_names: None,  query_id: Some(2), ones: Some(vec![0]), query_name: None },
        ];
        expected.sort_by_key(|x| *x.query_id.as_ref().unwrap());

        let mut data = RoaringBitmap::new();
        data.insert(0);
        data.insert(4);

        let targets = vec!["chr.fasta".as_bytes().to_vec(), "plasmid.fasta".as_bytes().to_vec()];
        let queries = vec!["ERR4035126.1".as_bytes().to_vec(), "ERR4035126.2".as_bytes().to_vec(), "ERR4035126.651903".as_bytes().to_vec(), "ERR4035126.7543".as_bytes().to_vec(), "ERR4035126.16".as_bytes().to_vec()];
        let n_queries = queries.len();
        let (header, _) = build_file_header_and_flags(&targets, n_queries, &"ERR4035126".as_bytes().to_vec(), &MetadataCompression::default()).unwrap();

        let mut tmp = data.iter().map(|x| x as u64);
        let mut bdecoder = BitmapDecoder::new(&mut tmp, header);

        let mut got: Vec<PseudoAln> = Vec::with_capacity(expected.len());
        for record in bdecoder.by_ref() {
            got.push(record);
        }
        got.sort_by_key(|x| *x.query_id.as_ref().unwrap());

        assert_eq!(got, expected);
    }
}
