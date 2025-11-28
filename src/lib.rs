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

//! ahda is a library and a command-line client for converting between
//! pseudoalignment formats output by different tools and for compressing the
//! data by up to 1000x compared to plaintext and 100x compared to gzip.
//!
//! ahda supports the following three operations:
//!   - `ahda cat` print input file(s) in another format.
//!   - `ahda decode` decompress pseudoalignment data to a supported format.
//!   - `ahda encode` compress pseudoalignment data from a supported format.
//!
//! ahda can read input data from the following formats:
//!   - [Bifrost](https://github.com/pmelsted/bifrost)
//!   - [Fulgor](https://github.com/jermp/fulgor)
//!   - [Metagraph](https://github.com/ratschlab/metagraph)
//!   - [SAM](https://samtools.github.io/hts-specs/SAMv1.pdf)
//!   - [Themisto](https://github.com/algbio/themisto)
//!
//! For details on each input format, see [Format]. We welcome contributions
//! implementing support for new tools but recommend first investigating whether
//! one of the existing formats fits your needs.
//!

use headers::file::FileHeader;
use headers::file::FileFlags;
use headers::block::BlockFlags;
use headers::block::read_block_header;
use headers::block::decode_block_flags;
use headers::file::read_file_header;

use parser::Parser;

use std::io::Read;
use std::io::Write;

use roaring::bitmap::RoaringBitmap;

pub mod headers;
pub mod decoder;
pub mod encoder;
pub mod pack;
pub mod parser;
pub mod printer;
pub mod unpack;

type E = Box<dyn std::error::Error>;

/// Supported formats
///
/// Encoded as a 16-bit integer in [FileHeader] with the following mapping:
///
///   - 0: Unknown
///   - 1: [Themisto](https://github.com/algbio/themisto)
///
#[non_exhaustive]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Format {
    #[default] // TODO more sensible default
    Bifrost,
    Fulgor,
    Metagraph,
    SAM,
    Themisto,
}


impl std::str::FromStr for Format {
    type Err = String; // Define an error type for parsing failures

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "bifrost" => Ok(Format::Bifrost),
            "fulgor" => Ok(Format::Fulgor),
            "metagraph" => Ok(Format::Metagraph),
            "sam" => Ok(Format::SAM),
            "themisto" => Ok(Format::Themisto),
            _ => Err(format!("'{}' is not a valid Format", s)),
        }
    }
}

#[non_exhaustive]
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PseudoAln{
    pub ones: Option<Vec<u32>>,
    pub ones_names: Option<Vec<String>>,
    pub query_id: Option<u32>,
    pub query_name: Option<String>,
}

pub fn cat<W: Write>(
    records: &[PseudoAln],
    flags: &FileFlags,
    header: &FileHeader,
    format: &Format,
    conn: &mut W,
) -> Result<(),E> {
    let mut records_iter = records.iter();
    let mut printer = printer::Printer::new(&mut records_iter, header.clone(), flags.clone(), format.clone());

    for bytes in printer.by_ref() {
        conn.write_all(&bytes).unwrap();
    }

    Ok(())
}

pub fn parse<R: Read>(
    conn: &mut R,
) -> Result<(Vec<PseudoAln>, Format), E> {
    let mut reader = Parser::new(conn)?;

    let mut res: Vec<PseudoAln> = Vec::new();
    for record in reader.by_ref() {
        res.push(record);
    }

    Ok((res, reader.format))
}

/// Encode pseudoalignments to something that implements [Write](std::io::Write).
pub fn encode_block<W: Write>(
    file_header: &FileHeader,
    records: &[PseudoAln],
    conn: &mut W,
) -> Result<(), E> {
    assert!(!records.is_empty());

    let packed = pack::pack(file_header, records)?;
    conn.write_all(&packed)?;
    conn.flush()?;

    Ok(())
}

/// Parse plain-text pseudoalignments from [Read](std::io::Read) and encode to [Write](std::io::Write).
pub fn encode_from_std_read_to_std_write<R: Read, W: Write>(
    targets: &[String],
    queries: &[String],
    sample_name: &str,
    conn_in: &mut R,
    conn_out: &mut W,
) -> Result<(), E> {
    let mut reader = crate::parser::Parser::new(conn_in)?;
    let mut encoder = encoder::Encoder::new(&mut reader, targets, queries, sample_name);

    let bytes = encoder.encode_header_and_flags().unwrap();
    conn_out.write_all(&bytes)?;
    while let Some(bytes) = encoder.next_block() {
        conn_out.write_all(&bytes)?;
    }
    conn_out.flush()?;
    Ok(())
}

/// Decode pseudoalignments from [Read](std::io::Read) and format them to [Write](std::io::Write).
pub fn decode_from_std_read_to_std_write<R: Read, W: Write>(
    out_format: Format,
    conn_in: &mut R,
    conn_out: &mut W,
) -> Result<(), E> {
    let decoder = decoder::Decoder::new(conn_in);

    let header = decoder.file_header().clone();
    let flags = decoder.file_flags().clone();
    for block in decoder {
        let mut block_iter = block.iter();
        let printer = printer::Printer::new(&mut block_iter, header.clone(), flags.clone(), out_format.clone());
        for line in printer {
            conn_out.write_all(&line)?;
        }
    }

    conn_out.flush()?;
    Ok(())
}

/// Decode a block of pseudoalignments from something that implements [Read](std::io::Read).
pub fn decode_block_from_std_read<R: Read>(
    file_flags: &FileFlags,
    conn: &mut R,
) -> Result<Vec<PseudoAln>, E> {
    let block_header = read_block_header(conn)?;
    let mut bytes: Vec<u8> = vec![0; block_header.deflated_len as usize];
    conn.read_exact(&mut bytes)?;
    unpack::unpack(&bytes, &block_header, file_flags)
}

/// Decode all pseudoalignments from something that implements [Read](std::io::Read).
pub fn decode_file_from_std_read<R: Read>(
    file_flags: &FileFlags,
    conn: &mut R,
) -> Result<Vec<PseudoAln>, E> {

    let file_header = read_file_header(conn).unwrap();

    let mut dump: Vec<u8> = vec![0; file_header.flags_len as usize];
    let _ = conn.read_exact(&mut dump);

    let mut res: Vec<PseudoAln> = Vec::with_capacity(file_header.n_queries as usize);
    while let Ok(block_header) = read_block_header(conn) {
        let mut bytes: Vec<u8> = vec![0; block_header.deflated_len as usize];
        conn.read_exact(&mut bytes)?;
        res.append(&mut unpack::unpack(&bytes, &block_header, file_flags)?);
    }

    todo!("Implement decode_file_from_std_read"); // This function is broken
}

/// Reads the full bitmap and combined block flags from a file
pub fn read_bitmap<R: Read>(
    conn: &mut R,
) -> Result<(RoaringBitmap, BlockFlags), E> {
    let mut bitmap = RoaringBitmap::new();

    let mut queries: Vec<String> = Vec::new();
    let mut query_ids: Vec<u32> = Vec::new();

    while let Ok(block_header) = read_block_header(conn) {
        let mut deflated_bytes: Vec<u8> = vec![0; block_header.deflated_len as usize];
        conn.read_exact(&mut deflated_bytes)?;

        let inflated_bytes = unpack::inflate_bytes(&deflated_bytes)?;
        let inflated_bytes = unpack::inflate_bytes(&inflated_bytes)?;

        let flags_bytes = &inflated_bytes[(block_header.block_len as usize)..inflated_bytes.len()];
        let bitmap_bytes = &inflated_bytes[0..(block_header.block_len as usize)];

        let mut block_flags = decode_block_flags(flags_bytes)?;
        queries.append(&mut block_flags.queries);
        query_ids.append(&mut block_flags.query_ids);

        let bitmap_deser = RoaringBitmap::deserialize_from(bitmap_bytes);
        bitmap |= bitmap_deser?;
    }

    let mut both: Vec<(u32, String)> = queries.iter().zip(query_ids.iter()).map(|(name, idx)| (*idx, name.to_string())).collect();
    both.sort_by_key(|x| x.0);
    let queries: Vec<String> = both.iter().map(|x| x.1.to_string()).collect();
    let query_ids: Vec<u32> = both.iter().map(|x| x.0).collect();

    Ok((bitmap, BlockFlags{ queries, query_ids }))
}

// Tests
#[cfg(test)]
mod tests {

    #[test]
    fn parse_themisto_output() {
        use crate::Format;
        use std::io::Cursor;
        use super::parse;
        use super::PseudoAln;

        let data: Vec<u8> = vec![b"128 0 7 11 3\n".to_vec(),
                                 b"7 3 2 1 0\n".to_vec(),
                                 b"8\n".to_vec(),
                                 b"0\n".to_vec(),
                                 b"1 4 2 9 7\n".to_vec(),
        ].concat();

        let expected = vec![
            PseudoAln{ones_names: None,  query_id: Some(128), ones: Some(vec![0, 7, 11, 3]), ..Default::default()},
            PseudoAln{ones_names: None,  query_id: Some(7),   ones: Some(vec![3, 2, 1, 0]), ..Default::default()},
            PseudoAln{ones_names: None,  query_id: Some(8),   ones: Some(vec![]), ..Default::default()},
            PseudoAln{ones_names: None,  query_id: Some(0),   ones: Some(vec![]), ..Default::default()},
            PseudoAln{ones_names: None,  query_id: Some(1),   ones: Some(vec![4, 2, 9, 7]), ..Default::default()},
        ];

        let mut input: Cursor<Vec<u8>> = Cursor::new(data);
        let (got, got_format) = parse(&mut input).unwrap();

        assert_eq!(got_format, Format::Themisto);
        assert_eq!(got, expected);
    }

    #[test]
    fn parse_fulgor_output() {
        use crate::Format;
        use std::io::Cursor;

        use super::parse;
        use super::PseudoAln;

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

        let mut input: Cursor<Vec<u8>> = Cursor::new(data);
        let (got, got_format) = parse(&mut input).unwrap();

        assert_eq!(got_format, Format::Fulgor);
        assert_eq!(got, expected);
    }

    #[test]
    fn parse_bifrost_output() {
        use crate::Format;
        use std::io::Cursor;

        use super::parse;
        use super::PseudoAln;

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

        let mut input: Cursor<Vec<u8>> = Cursor::new(data);
        let (got, got_format) = parse(&mut input).unwrap();

        assert_eq!(got_format, Format::Bifrost);
        assert_eq!(got, expected);
    }

    #[test]
    fn parse_metgraph_output() {
        use crate::Format;
        use std::io::Cursor;

        use super::parse;
        use super::PseudoAln;

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

        let mut input: Cursor<Vec<u8>> = Cursor::new(data);
        let (got, got_format) = parse(&mut input).unwrap();

        assert_eq!(got_format, Format::Metagraph);
    }

    #[test]
    fn parse_sam_output() {
        use crate::Format;
        use std::io::Cursor;

        use super::parse;
        use super::PseudoAln;

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
            PseudoAln{ query_id: None, query_name: Some("ERR4035126.1".to_string()), ones_names: Some(vec!["OZ038621.1".to_string()]), ones: Some(vec![]) },
            PseudoAln{ query_id: None, query_name: Some("ERR4035126.2".to_string()), ones_names: Some(vec!["OZ038621.1".to_string()]), ones: Some(vec![]) },
            PseudoAln{ query_id: None, query_name: Some("ERR4035126.3".to_string()), ones_names: Some(vec!["OZ038621.1".to_string()]), ones: Some(vec![]) },
            PseudoAln{ query_id: None, query_name: Some("ERR4035126.1261584".to_string()), ones_names: Some(vec!["OZ038622.1".to_string()]), ones: Some(vec![]) },
            PseudoAln{ query_id: None, query_name: Some("ERR4035126.1213410".to_string()), ones_names: Some(vec!["OZ038622.1".to_string()]), ones: Some(vec![]) },
            PseudoAln{ query_id: None, query_name: Some("ERR4035126.1213410".to_string()), ones_names: Some(vec!["OZ038621.1".to_string()]), ones: Some(vec![]) },
            PseudoAln{ query_id: None, query_name: Some("ERR4035126.4".to_string()), ones_names: Some(vec!["OZ038621.1".to_string()]), ones: Some(vec![]) },
            PseudoAln{ query_id: None, query_name: Some("ERR4035126.5".to_string()), ones_names: Some(vec!["OZ038621.1".to_string()]), ones: Some(vec![]) },
            PseudoAln{ query_id: None, query_name: Some("ERR4035126.6".to_string()), ones_names: Some(vec!["OZ038621.1".to_string()]), ones: Some(vec![]) },
            PseudoAln{ query_id: None, query_name: Some("ERR4035126.973529".to_string()), ones_names: Some(vec!["OZ038621.1".to_string()]), ones: Some(vec![]) },
            PseudoAln{ query_id: None, query_name: Some("ERR4035126.973529".to_string()), ones_names: Some(vec!["OZ038621.1".to_string()]), ones: Some(vec![]) },
            PseudoAln{ query_id: None, query_name: Some("ERR4035126.621281".to_string()), ones_names: Some(vec!["OZ038621.1".to_string()]), ones: Some(vec![]) },
            PseudoAln{ query_id: None, query_name: Some("ERR4035126.1178767".to_string()), ones_names: None, ones: None },
            PseudoAln{ query_id: None, query_name: Some("ERR4035126.621281".to_string()), ones_names: Some(vec!["OZ038621.1".to_string()]), ones: Some(vec![]) },
            PseudoAln{ query_id: None, query_name: Some("ERR4035126.621281".to_string()), ones_names: Some(vec!["OZ038621.1".to_string()]), ones: Some(vec![]) },
        ];

        let mut input: Cursor<Vec<u8>> = Cursor::new(data);
        let (got, got_format) = parse(&mut input).unwrap();

        assert_eq!(got_format, Format::SAM);

        got.iter().zip(expected.iter()).for_each(|(x, y)| { assert_eq!(x, y) });

        assert_eq!(got, expected);
    }
}
