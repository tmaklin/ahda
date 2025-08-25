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

use bstr::BString;
use indexmap::map::IndexMap;
use noodles_sam::{
    self as sam,
    alignment::io::Write,
    header::record::value::{map::ReferenceSequence, Map},
};

use crate::PseudoAln;

type E = Box<dyn std::error::Error>;

#[derive(Debug, Clone)]
pub struct SamPrinterError;

impl std::fmt::Display for SamPrinterError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "invalid input to encode")
    }
}

impl std::error::Error for SamPrinterError {}

/// Format a single pseudoalignment in Sam format
///
/// Writes bytes containing the formatted line containing the contents of
/// `aln` to `conn`.
///
/// Terminates with a [SamPrinterError] if [PseudoAln::query_id] or
/// [PseudoAln::ones] is None.
///
pub fn format_sam_line<W: std::io::Write>(
    aln: &PseudoAln,
    header: &sam::Header,
    conn: &mut W,
) -> Result<(), E> {
    if aln.ones.is_none() || aln.query_name.is_none() {
        return Err(Box::new(SamPrinterError{}))
    }

    let mut writer = noodles_sam::io::Writer::new(Vec::new());

    // TODO Error if query_name or ones is None

    for target_id in aln.ones.as_ref().unwrap() {
        let record = sam::alignment::RecordBuf::builder()
            .set_name(aln.query_name.clone().unwrap())
            .set_reference_sequence_id(*target_id as usize)
            .build();
        writer.write_alignment_record(header, &record)?;
    }
    conn.write_all(writer.get_ref())?;

    Ok(())
}

/// Builds a noodles_sam header
pub fn build_sam_header(
    targets: &[String],
    // file_header: &FileHeader,
    // file_flags: &FileFlags
) -> Result<sam::Header, E> {
    let refs = targets.iter().map(|target_name| {
        (
            BString::from(target_name.clone()),
            Map::<ReferenceSequence>::new(std::num::NonZeroUsize::try_from(1).unwrap()),
        )
    }).collect::<IndexMap<BString, Map<ReferenceSequence>>>();
    // builder.add_program("noodles-sam", Map::<Program>::default()) TODO match format and add
    // builder.add_comment("noodles-sam").build(); // TODO note that this was converted with ahda

    Ok(
        sam::Header::builder()
            .set_header(Default::default())
            .set_reference_sequences(refs)
            // .add_read_group(file_flags.query_name.clone(), Map::<ReadGroup>::default())
            .build()
    )
}

/// Formats a noodles_sam header
pub fn format_sam_header<W: std::io::Write>(
    header: &sam::Header,
    conn: &mut W,
) -> Result<(), E> {
    let mut writer = noodles_sam::io::Writer::new(Vec::new());
    writer.write_header(&header)?;
    conn.write_all(writer.get_ref())?;
    Ok(())
}

// Tests
#[cfg(test)]
mod tests {

    #[test]
    fn format_sam_line_aligned() {
        // use crate::headers::file::FileHeader;
        use crate::headers::file::FileFlags;
        use super::build_sam_header;
        use super::format_sam_line;
        use crate::PseudoAln;

        // Build header
        // let fheader = FileHeader { n_targets: 2, ..Default::default() };
        let fflags = FileFlags { target_names: vec!["chr.fasta".to_string(), "plasmid.fasta".to_string()], query_name: "test.fastq".to_string() };
        let mut expected: Vec<u8> = b"@HD\tVN:1.6\n".to_vec();
        expected.append(&mut b"@SQ\tSN:chr.fasta\tLN:1\n".to_vec());
        expected.append(&mut b"@SQ\tSN:plasmid.fasta\tLN:1\n".to_vec());
        expected.append(&mut b"@RG\tID:test.fastq\n".to_vec());
        // let header = build_sam_header(&fheader, &fflags).unwrap();
        let header = build_sam_header(&fflags.target_names).unwrap();

        let data = PseudoAln{ones_names: Some(vec!["OZ038621.1".to_string()]), query_id: None, ones: Some(vec![1]), query_name: Some("ERR4035126.1".to_string()) };

        let expected: Vec<u8> =b"ERR4035126.1\t4\tplasmid.fasta\t0\t255\t*\t*\t0\t0\t*\t*\n".to_vec();

        let mut got: Vec<u8> = Vec::new();
        format_sam_line(&data, &header, &mut got).unwrap();

        assert_eq!(got.iter().map(|x| *x as char).collect::<String>(), expected.iter().map(|x| *x as char).collect::<String>())
    }

    #[test]
    fn build_sam_header() {
        // use crate::headers::file::FileHeader;
        use crate::headers::file::FileFlags;
        use super::build_sam_header;
        use super::format_sam_header;

        // let fheader = FileHeader { n_targets: 2, ..Default::default() };
        let fflags = FileFlags { target_names: vec!["chr.fasta".to_string(), "plasmid.fasta".to_string()], query_name: "test.fastq".to_string() };

        let mut expected: Vec<u8> = b"@HD\tVN:1.6\n".to_vec();
        expected.append(&mut b"@SQ\tSN:chr.fasta\tLN:1\n".to_vec());
        expected.append(&mut b"@SQ\tSN:plasmid.fasta\tLN:1\n".to_vec());
        // expected.append(&mut b"@RG\tID:test.fastq\n".to_vec());

        // let header = build_sam_header(&fheader, &fflags).unwrap();
        let header = build_sam_header(&fflags.target_names).unwrap();

        let mut got: Vec<u8> = Vec::new();
        format_sam_header(&header, &mut got).unwrap();

        assert_eq!(got, expected)
    }
}
