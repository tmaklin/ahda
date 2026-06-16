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

//! Error types from the ahda library.

use crate::Format;

/// Not a valid [ahda library version](crate::AhdaVersion).
#[derive(Debug, Clone)]
pub struct AhdaVersionErr;
impl std::fmt::Display for AhdaVersionErr {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "Not a valid AhdaVersion")
    }
}
impl std::error::Error for AhdaVersionErr {}

/// Not a valid [ahda file format version](crate::AhdaFormatVersion).
#[derive(Debug, Clone)]
pub struct AhdaFormatVersionErr;
impl std::fmt::Display for AhdaFormatVersionErr {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "Not a valid AhdaFormatVersion")
    }
}
impl std::error::Error for AhdaFormatVersionErr {}

/// Binary data given to [check_ahda_header](crate::headers::file::check_ahda_header) does not start with the ahda header bytes.
#[derive(Debug, Clone)]
pub struct AhdaHeaderError;
impl std::fmt::Display for AhdaHeaderError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "bytes do not start with a valid ahda file header")
    }
}
impl std::error::Error for AhdaHeaderError {}

/// Set bits iterator given to [BitmapEncoder](crate::encoder::bitmap_encoder::BitmapEncoder) is not sorted.
#[derive(Debug, Clone)]
pub struct SetBitsIteratorNotSortedErr;
impl std::fmt::Display for SetBitsIteratorNotSortedErr {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "`set_bits` iterator given to BitmapEncoder::new() (argument #1) must be sorted.")
    }
}
impl std::error::Error for SetBitsIteratorNotSortedErr {}

/// Could not format [PseudoAln](crate::PseudoAln) as a Themisto plain text line.
#[derive(Debug, Clone)]
pub struct ThemistoPrinterError;
impl std::fmt::Display for ThemistoPrinterError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "invalid input to encode")
    }
}
impl std::error::Error for ThemistoPrinterError {}

/// Could not format [PseudoAln](crate::PseudoAln) as a SAM plain text line.
#[derive(Debug, Clone)]
pub struct SamPrinterError;
impl std::fmt::Display for SamPrinterError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "invalid input to encode")
    }
}
impl std::error::Error for SamPrinterError {}

/// Could not format [PseudoAln](crate::PseudoAln) as a Fulgor plain text line.
#[derive(Debug, Clone)]
pub struct FulgorPrinterError;
impl std::fmt::Display for FulgorPrinterError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "invalid input to encode")
    }
}
impl std::error::Error for FulgorPrinterError {}

/// Could not format [PseudoAln](crate::PseudoAln) as a Bifrost plain text line.
#[derive(Debug, Clone)]
pub struct BifrostPrinterError;
impl std::fmt::Display for BifrostPrinterError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "invalid input to encode")
    }
}
impl std::error::Error for BifrostPrinterError {}

/// Could not format [PseudoAln](crate::PseudoAln) as a Metagraph plain text line.
#[derive(Debug, Clone)]
pub struct MetagraphPrinterError;
impl std::fmt::Display for MetagraphPrinterError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "invalid input to encode")
    }
}
impl std::error::Error for MetagraphPrinterError {}

/// [guess_format](crate::parser::guess_format) did not recognize the input format given to [Parser](crate::parser::Parser).
#[derive(Debug, Clone)]
pub struct UnrecognizedInputFormatErr;
impl std::fmt::Display for UnrecognizedInputFormatErr {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "Unrecognized input format.")
    }
}
impl std::error::Error for UnrecognizedInputFormatErr {}

/// [guess_format](crate::parser::guess_format) found a valid input format to [Parser](crate::parser::Parser) but could not confirm it with certainty.
#[derive(Debug, Clone)]
pub struct AmbiguousInputFormatErr;
impl std::fmt::Display for AmbiguousInputFormatErr {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "Ambiguous input format.")
    }
}
impl std::error::Error for AmbiguousInputFormatErr {}

/// Plaintext data given to [Parser](crate::parser::Parser) is not in the expected format.
#[derive(Debug, Clone)]
pub struct CorruptedInputErr;
impl std::fmt::Display for CorruptedInputErr {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "Corrupted input alignment data.")
    }
}
impl std::error::Error for CorruptedInputErr {}

/// Input format given to [Parser](crate::parser::Parser) requires supplying the target sequence names.
#[derive(Debug, Clone)]
pub struct NeedTargetSequencesErr {
    pub format: Format,
}
impl std::fmt::Display for NeedTargetSequencesErr {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "Detected input format `{}` requires supplying the target sequence names.", self.format)
    }
}
impl std::error::Error for NeedTargetSequencesErr {}

/// Input format given to [Parser](crate::parser::Parser) requires supplying the query sequence names.
#[derive(Debug, Clone)]
pub struct NeedQueryNamesErr {
    pub format: Format,
}

impl std::fmt::Display for NeedQueryNamesErr {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "Detected input format `{}` requires supplying the query sequence names.", self.format)
    }
}

impl std::error::Error for NeedQueryNamesErr {}

/// A [PseudoAln](crate::PseudoAln) record given to encoder does not contain `query_id` and/or `ones`.
#[derive(Debug, Clone)]
pub struct EncodeError;
impl std::fmt::Display for EncodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "invalid input to encode")
    }
}
impl std::error::Error for EncodeError {}

/// Bifrost header line was not consumed before calling [read_bifrost](crate::parser::bifrost::read_bifrost).
#[derive(Debug, Clone)]
pub struct BifrostHeaderNotConsumedError;
impl std::fmt::Display for BifrostHeaderNotConsumedError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "Bifrost header not consumed from input `Read`.")
    }
}
impl std::error::Error for BifrostHeaderNotConsumedError {}

/// Duplicated queries in files being concatenated.
#[derive(Debug, Clone)]
pub struct DuplicatedQueriesErr;
impl std::fmt::Display for DuplicatedQueriesErr {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "Inputs contain duplicated query records, use a set operation to merge instead.")
    }
}
impl std::error::Error for DuplicatedQueriesErr {}

/// Duplicated queries in files being concatenated.
#[derive(Debug, Clone)]
pub struct IncompatibleFileHeadersErr;
impl std::fmt::Display for IncompatibleFileHeadersErr {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "Incompatible file headers.")
    }
}
impl std::error::Error for IncompatibleFileHeadersErr {}
