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

pub mod gzwrapper;
pub mod roaring;

use crate::PseudoAln;
use crate::headers::file::FileHeader;

use roaring::convert_to_roaring;
use roaring::pack_block_roaring;

type E = Box<dyn std::error::Error>;

/// Supported bitmap types for an .ahda record
#[non_exhaustive]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum BitmapType {
    /// RoaringBitmap (32-bit address space)
    #[default]
    Roaring32,
    /// RoaringTreemap (64-bit address space)
    Roaring64,
}


impl BitmapType {
    fn from_u16(val: u16) -> Result<Self, E> {
        match val {
            0 => Ok(BitmapType::Roaring32),
            1 => Ok(BitmapType::Roaring64),
            _ => panic!("Not a valid BitmapType"),
        }
    }
}

pub fn pack_records(
    file_header: &FileHeader,
    records: &[PseudoAln],
) -> Result<Vec<u8>, E> {
    let queries: Vec<String> = records.iter().filter_map(|record| {
        assert!(record.query_name.is_some());
        record.query_name.clone()
    }).collect();

    let query_ids: Vec<u32> = records.iter().filter_map(|record| {
        assert!(record.query_id.is_some());
        record.query_id
    }).collect();

    let block = match BitmapType::from_u16(file_header.bitmap_type)? {
        BitmapType::Roaring32 => {
            let bitmap = convert_to_roaring(file_header, records)?;
            pack_block_roaring(&queries, &query_ids, &bitmap)?
        },
        BitmapType::Roaring64 => {
            todo!("converting records to a RoaringTreemap");
        }
    };

    Ok(block)
}
