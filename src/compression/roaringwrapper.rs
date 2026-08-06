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

//! Roaring treemap wrapper (64-bit address space).

use crate::PseudoAln;
use crate::headers::block::BlockFlags;
use crate::headers::block::BlockHeader;
use crate::headers::file::FileHeader;
use crate::headers::block::encode_block_header;
use crate::headers::block::encode_block_flags;
use crate::headers::block::decode_block_flags;

use crate::compression::gzwrapper::deflate_bytes;
use crate::compression::gzwrapper::inflate_bytes;

use super::BitmapType;
use super::BitmapHolder;
use super::MetadataCompression;

use roaring::bitmap::RoaringBitmap;
use roaring::treemap::RoaringTreemap;

type E = Box<dyn std::error::Error>;

/// Converts [PseudoAln] records to a BitmapHolder holding the bitmap type specified in [FileHeader]
pub fn convert_to_roaring(
    file_header: &FileHeader,
    records: Vec<PseudoAln>,
) -> Result<BitmapHolder, E> {
    let n_targets: u64 = file_header.n_targets as u64;

    let mut bits = match BitmapType::from_u16(file_header.bitmap_type)? {
        BitmapType::Roaring32 => BitmapHolder::Roaring32(RoaringBitmap::new()),
        BitmapType::Roaring64 => BitmapHolder::Roaring64(RoaringTreemap::new()),
    };

    for record in records.iter() {
        let ones = if let Some(indexes) = &record.ones {
            indexes
        } else {
            return Err(Box::new(crate::errors::PseudoAlnOnesIsNone{}))
        };

        let idx = if let Some(query_id) = record.query_id {
            query_id
        } else {
            return Err(Box::new(crate::errors::PseudoAlnQueryIdIsNone))
        };

        match &mut bits {
            BitmapHolder::Roaring32(bits) => {
                ones.iter().for_each(|bit_idx| {
                    let index = idx as u32 *n_targets as u32 + *bit_idx;
                    bits.insert(index);
                });
            },
            BitmapHolder::Roaring64(bits) => {
                ones.iter().map(|x| *x as u64).for_each(|bit_idx| {
                    let index = idx as u64 * n_targets + bit_idx;
                    bits.insert(index);
                });
            }
        }
    }

    Ok(bits)
}

pub fn serialize_roaring(
    bits: BitmapHolder,
) -> Result<Vec<u8>, E> {
    let mut bytes: Vec<u8> = Vec::new();
    match bits {
        BitmapHolder::Roaring32(bits) => bits.serialize_into(&mut bytes)?,
        BitmapHolder::Roaring64(bits) => bits.serialize_into(&mut bytes)?,
    }
    let bytes = deflate_bytes(&bytes)?;
    Ok(bytes)
}

pub fn deserialize_roaring32(
    bytes: &[u8],
) -> Result<RoaringBitmap, E> {
    let bitmap_bytes = inflate_bytes(bytes)?;
    let bitmap = RoaringBitmap::deserialize_from(bitmap_bytes.as_slice())?;
    Ok(bitmap)
}

pub fn deserialize_roaring64(
    bytes: &[u8],
) -> Result<RoaringTreemap, E> {
    let bitmap_bytes = inflate_bytes(bytes)?;
    let bitmap = RoaringTreemap::deserialize_from(bitmap_bytes.as_slice())?;
    Ok(bitmap)
}

pub fn pack_block_roaring(
    query_ids: &[u32],
    queries: Option<Vec<Vec<u8>>>,
    bitmap: BitmapHolder,
) -> Result<Vec<u8>, E> {
    let bitmap_type = match bitmap {
        BitmapHolder::Roaring32(_) => BitmapType::Roaring32,
        BitmapHolder::Roaring64(_) => BitmapType::Roaring64,
    };

    let mut serialized = serialize_roaring(bitmap)?;

    let n_queries = query_ids.len();
    if let Some(query_names) = &queries {
        assert_eq!(query_ids.len(), query_names.len());
    }

    let flags: BlockFlags = BlockFlags{ queries, query_ids: Some(query_ids.to_vec()) };
    let fields_present = flags.fields_present();
    let mut block_flags: Vec<u8> = encode_block_flags(&flags)?;

    let flags_len = block_flags.len() as u64;
    let block_len = serialized.len() as u32;

    let header = BlockHeader{
        num_records: n_queries as u32,
        block_len,
        flags_len,
        bitmap_type: bitmap_type.to_u16(),
        metadata_compression: MetadataCompression::default().to_u8(),
        fields_present,
        placeholder1: 0,
        placeholder2: 0,
        placeholder3: 0,
    };

    let mut block: Vec<u8> = encode_block_header(&header)?;
    block.append(&mut block_flags);
    block.append(&mut serialized);

    Ok(block)
}

pub fn unpack_block_roaring(
    bytes: &[u8],
    block_header: &BlockHeader,
) -> Result<(BitmapHolder, BlockFlags), E> {
    let block_flags = decode_block_flags(&bytes[0..(block_header.flags_len as usize)])?;

    let start_idx: usize = block_header.flags_len.try_into()?;
    let block_len: u64 = block_header.block_len as u64;
    let end_idx: usize = (block_header.flags_len + block_len).try_into()?;

    let block_bytes = &bytes[start_idx..end_idx];
    let bitmap = match BitmapType::from_u16(block_header.bitmap_type)? {
        BitmapType::Roaring32 => BitmapHolder::Roaring32(deserialize_roaring32(block_bytes)?),
        BitmapType::Roaring64 => BitmapHolder::Roaring64(deserialize_roaring64(block_bytes)?),
    };

    Ok((bitmap, block_flags))
}

#[cfg(test)]
mod tests {

    #[test]
    fn convert_to_roaring32() {
        use super::convert_to_roaring;
        use crate::compression::BitmapHolder;
        use crate::PseudoAln;
        use crate::AhdaFormatVersion;
        use crate::BitmapType;
        use crate::MetadataCompression;
        use crate::headers::file::FileHeader;
        use crate::headers::file::build_ahda_header;
        use roaring::bitmap::RoaringBitmap;

        let data = vec![
            PseudoAln{ones_names: Some(vec!["chr.fasta".as_bytes().to_vec()]),  query_id: Some(1), ones: Some(vec![0]), query_name: Some("ERR4035126.2".as_bytes().to_vec()) },
            PseudoAln{ones_names: Some(vec!["chr.fasta".as_bytes().to_vec()]),  query_id: Some(0), ones: Some(vec![0]), query_name: Some("ERR4035126.1".as_bytes().to_vec()) },
            PseudoAln{ones_names: Some(vec!["chr.fasta".as_bytes().to_vec(), "plasmid.fasta".as_bytes().to_vec()]),  query_id: Some(2), ones: Some(vec![0, 1]), query_name: Some("ERR4035126.651903".as_bytes().to_vec()) },
            PseudoAln{ones_names: Some(vec![]),  query_id: Some(4), ones: Some(vec![]), query_name: Some("ERR4035126.16".as_bytes().to_vec()) },
            PseudoAln{ones_names: Some(vec!["plasmid.fasta".as_bytes().to_vec()]),  query_id: Some(3), ones: Some(vec![1]), query_name: Some("ERR4035126.7543".as_bytes().to_vec()) },
        ];

        let header = FileHeader {
            ahda_header: build_ahda_header().unwrap(),
            file_format: AhdaFormatVersion::V1_0_0.to_u8(),
            metadata_compression: MetadataCompression::default().to_u8(),
            fields_present: crate::MASK_QUERY_IDS | crate::MASK_QUERIES,
            n_targets: 2_u32,
            n_queries: 5_u32,
            bitmap_type: BitmapType::Roaring32.to_u16(),
            block_size: 1000_u32,
            flags_len: 0_u64,
        };

        let aligned_indexes = vec![0_u32, 2, 4, 5, 7];

        let mut expected_bitmap = RoaringBitmap::new();
        aligned_indexes.into_iter().for_each(|val| { expected_bitmap.insert(val); });
        let expected = BitmapHolder::Roaring32(expected_bitmap);

        let got = convert_to_roaring(&header, data).unwrap();

        assert_eq!(got, expected);
    }

    #[test]
    fn convert_to_roaring64() {
        use super::convert_to_roaring;
        use crate::compression::BitmapHolder;
        use crate::PseudoAln;
        use crate::AhdaFormatVersion;
        use crate::BitmapType;
        use crate::MetadataCompression;
        use crate::headers::file::FileHeader;
        use crate::headers::file::build_ahda_header;
        use roaring::treemap::RoaringTreemap;

        let data = vec![
            PseudoAln{ones_names: Some(vec!["chr.fasta".as_bytes().to_vec()]),  query_id: Some(1), ones: Some(vec![0]), query_name: Some("ERR4035126.2".as_bytes().to_vec()) },
            PseudoAln{ones_names: Some(vec!["chr.fasta".as_bytes().to_vec()]),  query_id: Some(0), ones: Some(vec![0]), query_name: Some("ERR4035126.1".as_bytes().to_vec()) },
            PseudoAln{ones_names: Some(vec!["chr.fasta".as_bytes().to_vec(), "plasmid.fasta".as_bytes().to_vec()]),  query_id: Some(2), ones: Some(vec![0, 1]), query_name: Some("ERR4035126.651903".as_bytes().to_vec()) },
            PseudoAln{ones_names: Some(vec![]),  query_id: Some(4), ones: Some(vec![u32::MAX / 2]), query_name: Some("ERR4035126.16".as_bytes().to_vec()) },
            PseudoAln{ones_names: Some(vec!["plasmid.fasta".as_bytes().to_vec()]),  query_id: Some(3), ones: Some(vec![1]), query_name: Some("ERR4035126.7543".as_bytes().to_vec()) },
        ];

        let header = FileHeader {
            ahda_header: build_ahda_header().unwrap(),
            file_format: AhdaFormatVersion::V1_0_0.to_u8(),
            metadata_compression: MetadataCompression::default().to_u8(),
            fields_present: crate::MASK_QUERY_IDS | crate::MASK_QUERIES,
            n_targets: 2_u32 + u32::MAX / 2,
            n_queries: 5_u32,
            bitmap_type: BitmapType::Roaring64.to_u16(),
            block_size: 1000_u32,
            flags_len: 0_u64,
        };

        let aligned_indexes = vec![0_u64, 2147483649, 4294967298, 4294967299, 6442450948, 10737418243];

        let mut expected_bitmap = RoaringTreemap::new();
        aligned_indexes.into_iter().for_each(|val| { expected_bitmap.insert(val); });
        let expected = BitmapHolder::Roaring64(expected_bitmap);

        let got = convert_to_roaring(&header, data).unwrap();

        assert_eq!(got, expected);
    }

    #[test]
    fn serialize_roaring32() {
        use super::serialize_roaring;
        use crate::compression::BitmapHolder;
        use roaring::bitmap::RoaringBitmap;

        let aligned_indexes = vec![0_u32, 2, 4, 5, 7];
        let mut bitmap = RoaringBitmap::new();
        aligned_indexes.into_iter().for_each(|val| { bitmap.insert(val); });
        let data = BitmapHolder::Roaring32(bitmap);

        let expected: Vec<u8> = vec![31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 179, 50, 96, 96, 96, 100, 0, 1, 22, 6, 1, 48, 205, 196, 192, 194, 192, 202, 192, 206, 0, 0, 47, 109, 177, 38, 26, 0, 0, 0];

        let got = serialize_roaring(data).unwrap();

        assert_eq!(got, expected);
    }

    #[test]
    fn serialize_roaring64() {
        use super::serialize_roaring;
        use crate::compression::BitmapHolder;
        use roaring::treemap::RoaringTreemap;

        let aligned_indexes = vec![0_u64, 2147483649, 4294967298, 4294967299, 6442450948, 10737418243];
        let mut bitmap = RoaringTreemap::new();
        aligned_indexes.into_iter().for_each(|val| { bitmap.insert(val); });
        let data = BitmapHolder::Roaring64(bitmap);

        let expected: Vec<u8> = vec![31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 85, 138, 65, 14, 0, 16, 16, 196, 186, 214, 3, 28, 69, 60, 194, 217, 235, 60, 93, 102, 19, 196, 220, 58, 173, 243, 54, 7, 164, 3, 11, 42, 208, 2, 12, 251, 188, 93, 223, 209, 231, 228, 48, 42, 84, 202, 22, 192, 217, 60, 192, 39, 99, 96, 0, 0, 0];

        let got = serialize_roaring(data).unwrap();

        assert_eq!(got, expected);
    }

    #[test]
    fn deserialize_roaring32() {
        use super::deserialize_roaring32;
        use roaring::bitmap::RoaringBitmap;

        let aligned_indexes = vec![0_u32, 2, 4, 5, 7];
        let mut expected = RoaringBitmap::new();
        aligned_indexes.into_iter().for_each(|val| { expected.insert(val); });

        let data: Vec<u8> = vec![31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 179, 50, 96, 96, 96, 100, 0, 1, 22, 6, 1, 48, 205, 196, 192, 194, 192, 202, 192, 206, 0, 0, 47, 109, 177, 38, 26, 0, 0, 0];

        let got = deserialize_roaring32(&data).unwrap();

        assert_eq!(got, expected);
    }

    #[test]
    fn deserialize_roaring64() {
        use super::deserialize_roaring64;
        use roaring::treemap::RoaringTreemap;

        let aligned_indexes = vec![0_u64, 2147483649, 4294967298, 4294967299, 6442450948, 10737418243];
        let mut expected = RoaringTreemap::new();
        aligned_indexes.into_iter().for_each(|val| { expected.insert(val); });

        let data: Vec<u8> = vec![31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 85, 138, 65, 14, 0, 16, 16, 196, 186, 214, 3, 28, 69, 60, 194, 217, 235, 60, 93, 102, 19, 196, 220, 58, 173, 243, 54, 7, 164, 3, 11, 42, 208, 2, 12, 251, 188, 93, 223, 209, 231, 228, 48, 42, 84, 202, 22, 192, 217, 60, 192, 39, 99, 96, 0, 0, 0];

        let got = deserialize_roaring64(&data).unwrap();

        assert_eq!(got, expected);
    }

    #[test]
    fn pack_block_roaring32() {
        use super::pack_block_roaring;
        use crate::compression::BitmapHolder;
        use roaring::bitmap::RoaringBitmap;

        let aligned_indexes = vec![0_u32, 2, 4, 5, 7];

        let mut data_bitmap = RoaringBitmap::new();
        aligned_indexes.into_iter().for_each(|val| { data_bitmap.insert(val); });
        let data = BitmapHolder::Roaring32(data_bitmap);

        let query_ids = vec![1_u32, 0, 2, 4, 3];
        let query_names: Vec<Vec<u8>> = vec![
            "ERR4035126.2".as_bytes().to_vec(),
            "ERR4035126.1".as_bytes().to_vec(),
            "ERR4035126.651903".as_bytes().to_vec(),
            "ERR4035126.16".as_bytes().to_vec(),
            "ERR4035126.7543".as_bytes().to_vec(),
        ];

        let expected: Vec<u8> = vec![5, 0, 0, 0, 0, 0, 0, 0, 40, 0, 0, 0, 64, 0, 0, 0, 0, 0, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 99, 100, 229, 113, 13, 10, 50, 49, 48, 54, 53, 52, 50, 211, 51, 66, 230, 24, 10, 34, 113, 204, 76, 13, 45, 13, 140, 121, 145, 165, 205, 248, 145, 120, 230, 166, 38, 198, 140, 172, 140, 12, 76, 44, 204, 0, 68, 178, 157, 37, 83, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 179, 50, 96, 96, 96, 100, 0, 1, 22, 6, 1, 48, 205, 196, 192, 194, 192, 202, 192, 206, 0, 0, 47, 109, 177, 38, 26, 0, 0, 0];

        let got = pack_block_roaring(&query_ids, Some(query_names), data).unwrap();

        assert_eq!(got, expected);
    }

    #[test]
    fn pack_block_roaring64() {
        use super::pack_block_roaring;
        use crate::compression::BitmapHolder;
        use roaring::treemap::RoaringTreemap;

        let aligned_indexes = vec![0_u64, 2147483649, 4294967298, 4294967299, 6442450948, 10737418243];

        let mut data_bitmap = RoaringTreemap::new();
        aligned_indexes.into_iter().for_each(|val| { data_bitmap.insert(val); });
        let data = BitmapHolder::Roaring64(data_bitmap);

        let query_ids = vec![1_u32, 0, 2, 4, 3];
        let query_names: Vec<Vec<u8>> = vec![
            "ERR4035126.2".as_bytes().to_vec(),
            "ERR4035126.1".as_bytes().to_vec(),
            "ERR4035126.651903".as_bytes().to_vec(),
            "ERR4035126.16".as_bytes().to_vec(),
            "ERR4035126.7543".as_bytes().to_vec(),
        ];

        let expected: Vec<u8> = vec![5, 0, 0, 0, 0, 1, 0, 0, 67, 0, 0, 0, 64, 0, 0, 0, 0, 0, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 99, 100, 229, 113, 13, 10, 50, 49, 48, 54, 53, 52, 50, 211, 51, 66, 230, 24, 10, 34, 113, 204, 76, 13, 45, 13, 140, 121, 145, 165, 205, 248, 145, 120, 230, 166, 38, 198, 140, 172, 140, 12, 76, 44, 204, 0, 68, 178, 157, 37, 83, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 85, 138, 65, 14, 0, 16, 16, 196, 186, 214, 3, 28, 69, 60, 194, 217, 235, 60, 93, 102, 19, 196, 220, 58, 173, 243, 54, 7, 164, 3, 11, 42, 208, 2, 12, 251, 188, 93, 223, 209, 231, 228, 48, 42, 84, 202, 22, 192, 217, 60, 192, 39, 99, 96, 0, 0, 0];

        let got = pack_block_roaring(&query_ids, Some(query_names), data).unwrap();

        assert_eq!(got, expected);
    }

    #[test]
    fn pack_block_roaring_without_query_names() {
        use super::pack_block_roaring;
        use crate::compression::BitmapHolder;
        use roaring::bitmap::RoaringBitmap;

        let aligned_indexes = vec![0_u32, 2, 4, 5, 7];

        let mut data_bitmap = RoaringBitmap::new();
        aligned_indexes.into_iter().for_each(|val| { data_bitmap.insert(val); });
        let data = BitmapHolder::Roaring32(data_bitmap);

        let query_ids = vec![1_u32, 0, 2, 4, 3];

        let expected: Vec<u8> = vec![5, 0, 0, 0, 0, 0, 0, 0, 40, 0, 0, 0, 28, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 99, 96, 100, 101, 100, 96, 98, 97, 6, 0, 14, 44, 25, 80, 8, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 179, 50, 96, 96, 96, 100, 0, 1, 22, 6, 1, 48, 205, 196, 192, 194, 192, 202, 192, 206, 0, 0, 47, 109, 177, 38, 26, 0, 0, 0];

        let got = pack_block_roaring(&query_ids, None, data).unwrap();

        assert_eq!(got, expected);
    }

    #[test]
    fn unpack_block_roaring32() {
        use super::unpack_block_roaring;
        use crate::compression::BitmapHolder;
        use crate::headers::block::BlockHeader;
        use crate::headers::block::BlockFlags;

        use roaring::bitmap::RoaringBitmap;

        let aligned_indexes = vec![0_u32, 2, 4, 5, 7];

        let mut bitmap_data = RoaringBitmap::new();
        aligned_indexes.into_iter().for_each(|val| { bitmap_data.insert(val); });
        let expected_bitmap = BitmapHolder::Roaring32(bitmap_data);
        let expected_flags = BlockFlags {
            queries: Some(
                vec![
                    vec![69, 82, 82, 52, 48, 51, 53, 49, 50, 54, 46, 50],
                    vec![69, 82, 82, 52, 48, 51, 53, 49, 50, 54, 46, 49],
                    vec![69, 82, 82, 52, 48, 51, 53, 49, 50, 54, 46, 54, 53, 49, 57, 48, 51],
                    vec![69, 82, 82, 52, 48, 51, 53, 49, 50, 54, 46, 49, 54],
                    vec![69, 82, 82, 52, 48, 51, 53, 49, 50, 54, 46, 55, 53, 52, 51]
                ]
            ),
            query_ids: Some(vec![1, 0, 2, 4, 3]),
        };

        let block_header = BlockHeader {
            num_records: 5,
            metadata_compression: 0,
            bitmap_type: 0,
            placeholder1: 0,
            block_len: 40,
            flags_len: 64,
            fields_present: 3,
            placeholder2: 0,
            placeholder3: 0
        };

        let data: Vec<u8> = vec![5, 0, 0, 0, 0, 0, 0, 0, 40, 0, 0, 0, 64, 0, 0, 0, 0, 0, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 99, 100, 229, 113, 13, 10, 50, 49, 48, 54, 53, 52, 50, 211, 51, 66, 230, 24, 10, 34, 113, 204, 76, 13, 45, 13, 140, 121, 145, 165, 205, 248, 145, 120, 230, 166, 38, 198, 140, 172, 140, 12, 76, 44, 204, 0, 68, 178, 157, 37, 83, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 179, 50, 96, 96, 96, 100, 0, 1, 22, 6, 1, 48, 205, 196, 192, 194, 192, 202, 192, 206, 0, 0, 47, 109, 177, 38, 26, 0, 0, 0];

        let (got_bitmap, got_flags) = unpack_block_roaring(&data[32..data.len()], &block_header).unwrap();

        assert_eq!(got_bitmap, expected_bitmap);
        assert_eq!(got_flags, expected_flags);
    }

    #[test]
    fn unpack_block_roaring64() {
        use super::unpack_block_roaring;
        use crate::compression::BitmapHolder;
        use crate::headers::block::BlockHeader;
        use crate::headers::block::BlockFlags;

        use roaring::treemap::RoaringTreemap;

        let aligned_indexes = vec![0_u64, 2147483649, 4294967298, 4294967299, 6442450948, 10737418243];

        let mut bitmap_data = RoaringTreemap::new();
        aligned_indexes.into_iter().for_each(|val| { bitmap_data.insert(val); });
        let expected_bitmap = BitmapHolder::Roaring64(bitmap_data);
        let expected_flags = BlockFlags {
            queries: Some(
                vec![
                    vec![69, 82, 82, 52, 48, 51, 53, 49, 50, 54, 46, 50],
                    vec![69, 82, 82, 52, 48, 51, 53, 49, 50, 54, 46, 49],
                    vec![69, 82, 82, 52, 48, 51, 53, 49, 50, 54, 46, 54, 53, 49, 57, 48, 51],
                    vec![69, 82, 82, 52, 48, 51, 53, 49, 50, 54, 46, 49, 54],
                    vec![69, 82, 82, 52, 48, 51, 53, 49, 50, 54, 46, 55, 53, 52, 51]
                ]
            ),
            query_ids: Some(vec![1, 0, 2, 4, 3]),
        };

        let block_header = BlockHeader {
            num_records: 5,
            metadata_compression: 0,
            bitmap_type: 1,
            placeholder1: 0,
            block_len: 67,
            flags_len: 64,
            fields_present: 3,
            placeholder2: 0,
            placeholder3: 0
        };

        let data: Vec<u8> = vec![5, 0, 0, 0, 0, 1, 0, 0, 67, 0, 0, 0, 64, 0, 0, 0, 0, 0, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 99, 100, 229, 113, 13, 10, 50, 49, 48, 54, 53, 52, 50, 211, 51, 66, 230, 24, 10, 34, 113, 204, 76, 13, 45, 13, 140, 121, 145, 165, 205, 248, 145, 120, 230, 166, 38, 198, 140, 172, 140, 12, 76, 44, 204, 0, 68, 178, 157, 37, 83, 0, 0, 0, 31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 85, 138, 65, 14, 0, 16, 16, 196, 186, 214, 3, 28, 69, 60, 194, 217, 235, 60, 93, 102, 19, 196, 220, 58, 173, 243, 54, 7, 164, 3, 11, 42, 208, 2, 12, 251, 188, 93, 223, 209, 231, 228, 48, 42, 84, 202, 22, 192, 217, 60, 192, 39, 99, 96, 0, 0, 0];

        let (got_bitmap, got_flags) = unpack_block_roaring(&data[32..data.len()], &block_header).unwrap();

        assert_eq!(got_bitmap, expected_bitmap);
        assert_eq!(got_flags, expected_flags);
    }

}
