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
use std::io::Write;

use bstr::BString;
use indexmap::map::IndexMap;
use noodles_sam::{
    self as sam,
    header::record::value::{map::ReadGroup, map::ReferenceSequence, Map},
};

use crate::PseudoAln;
use crate::headers::file::FileFlags;
use crate::headers::file::FileHeader;

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
pub fn format_sam_line<W: Write>(
    aln: &PseudoAln,
    conn: &mut W,
) -> Result<(), E> {
    if aln.ones.is_none() || aln.query_name.is_none() {
        return Err(Box::new(SamPrinterError{}))
    }

    for target_id in aln.ones.as_ref().unwrap() {
        let record = sam::alignment::RecordBuf::builder()
            .set_name(aln.query_name.clone().unwrap())
            .set_reference_sequence_id(*target_id as usize)
            .build();
    }
    // See https://docs.rs/noodles-sam/latest/noodles_sam/io/writer/struct.Writer.html
    // for more details on how to continue from here
    Ok(())
}

/// Formats FileHeader + FileFlags as a SAM header
pub fn format_sam_header(
    file_header: &FileHeader,
    file_flags: &FileFlags
) -> Result<sam::Header, E> {
    let refs = file_flags.target_names.iter().map(|target_name| {
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
            .add_read_group(file_flags.query_name.clone(), Map::<ReadGroup>::default())
            .build()
    )
}


// Tests
#[cfg(test)]
mod tests {

    #[test]
    fn format_sam_line_aligned() {
        use crate::PseudoAln;
        use super::format_sam_line;

        let data = PseudoAln{ones_names: Some(vec!["OZ038621.1".to_string()]), query_id: None, ones: Some(vec![]), query_name: Some("ERR4035126.1".to_string()) };

        let expected: Vec<u8> =b"ERR4035126.1\t16\tOZ038621.1\t4541508\t60\t151M\t*\t0\t0\tAGTATTTAGTGACCTAAGTCAATAAAATTTTAATTTACTCACGGCAGGTAACCAGTTCAGAAGCTGCTATCAGACACTCTTTTTTTAATCCACACAGAGACATATTGCCCGTTGCAGTCAGAATGAAAAGCTGAAAATCACTTACTAAGGC FJ<<JJFJAA<-JFAJFAF<JFFJJJJJJJFJFJJA<A<AJJAAAFFJJJJFJJFJFJAJJ7JJJJJFJJJJJFFJFFJFJJJJJJFJ7FFJAJJJJJJJJFJJFJJFJFJJJJFJJFJJJJJJJJJFFJJJJJJJJJJJJJFJJJFFAAA\tNM:i:0\tMD:Z:151\tAS:i:151\tXS:i:0\n".to_vec();

        let mut got: Vec<u8> = Vec::new();
        format_sam_line(&data, &mut got).unwrap();

        assert_eq!(got, expected)
    }

}
