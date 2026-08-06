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

//! Flate2 wrapper.

use flate2::write::GzEncoder;
use flate2::write::GzDecoder;
use flate2::Compression;

use std::io::Write;

type E = Box<dyn std::error::Error>;

pub fn deflate_bytes(
    bytes: &[u8],
) -> Result<Vec<u8>, E> {
    let mut deflated: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut encoder = GzEncoder::new(&mut deflated, Compression::default());
    encoder.write_all(bytes)?;
    encoder.finish()?;
    Ok(deflated)
}

pub fn inflate_bytes(
    deflated: &[u8],
) -> Result<Vec<u8>, E> {
    let mut inflated: Vec<u8> = Vec::new();
    let mut decoder = GzDecoder::new(&mut inflated);
    decoder.write_all(deflated)?;
    decoder.finish()?;
    Ok(inflated)
}

#[cfg(test)]
mod tests {

    #[test]
    fn deflate_bytes() {
        use super::deflate_bytes;
        let data: Vec<u8> = b"0ml3hsMnl0Fz55nCj939O5SI8ootF7VMPOen00WmwxQ0eE1wIQ0s/v4wR5L+Q7p7xdIEuRIfUR+3MoDljQhRJCc9khL2cFaCINRD27YuvHim4dQu6yN7kQkSujtX53fPI8CNt7J/9JTFqFT3dRjV+tHadp6PbvAuti4ZI7T06hMs".to_vec();
        let expected: Vec<u8> = vec![31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 5, 193, 201, 18, 67, 48, 0, 0, 208, 15, 114, 144, 138, 8, 199, 14, 50, 98, 108, 9, 213, 229, 166, 150, 177, 211, 145, 160, 253, 250, 190, 7, 166, 17, 182, 91, 56, 143, 128, 252, 16, 154, 237, 222, 130, 86, 140, 82, 106, 46, 139, 32, 56, 15, 147, 184, 158, 1, 184, 79, 199, 201, 64, 237, 94, 14, 202, 192, 166, 238, 250, 193, 81, 160, 48, 188, 226, 179, 162, 174, 228, 180, 185, 113, 5, 134, 139, 51, 246, 172, 229, 190, 93, 90, 67, 27, 104, 37, 41, 108, 26, 113, 71, 195, 79, 185, 123, 221, 164, 87, 76, 26, 223, 8, 15, 108, 72, 101, 47, 30, 8, 54, 9, 53, 237, 72, 96, 95, 181, 252, 140, 124, 72, 6, 43, 222, 231, 138, 240, 138, 106, 53, 146, 247, 126, 149, 162, 211, 95, 20, 103, 192, 104, 195, 237, 15, 231, 143, 162, 114, 172, 0, 0, 0];
        let got = deflate_bytes(&data).unwrap();
        assert_eq!(got, expected);
    }

    #[test]
    fn inflate_bytes() {
        use super::inflate_bytes;
        let data: Vec<u8> = vec![31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 5, 193, 201, 18, 67, 48, 0, 0, 208, 15, 114, 144, 138, 8, 199, 14, 50, 98, 108, 9, 213, 229, 166, 150, 177, 211, 145, 160, 253, 250, 190, 7, 166, 17, 182, 91, 56, 143, 128, 252, 16, 154, 237, 222, 130, 86, 140, 82, 106, 46, 139, 32, 56, 15, 147, 184, 158, 1, 184, 79, 199, 201, 64, 237, 94, 14, 202, 192, 166, 238, 250, 193, 81, 160, 48, 188, 226, 179, 162, 174, 228, 180, 185, 113, 5, 134, 139, 51, 246, 172, 229, 190, 93, 90, 67, 27, 104, 37, 41, 108, 26, 113, 71, 195, 79, 185, 123, 221, 164, 87, 76, 26, 223, 8, 15, 108, 72, 101, 47, 30, 8, 54, 9, 53, 237, 72, 96, 95, 181, 252, 140, 124, 72, 6, 43, 222, 231, 138, 240, 138, 106, 53, 146, 247, 126, 149, 162, 211, 95, 20, 103, 192, 104, 195, 237, 15, 231, 143, 162, 114, 172, 0, 0, 0];
        let expected: Vec<u8> = b"0ml3hsMnl0Fz55nCj939O5SI8ootF7VMPOen00WmwxQ0eE1wIQ0s/v4wR5L+Q7p7xdIEuRIfUR+3MoDljQhRJCc9khL2cFaCINRD27YuvHim4dQu6yN7kQkSujtX53fPI8CNt7J/9JTFqFT3dRjV+tHadp6PbvAuti4ZI7T06hMs".to_vec();
        let got = inflate_bytes(&data).unwrap();
        assert_eq!(got, expected);
    }
}
