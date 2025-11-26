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
use ahda::headers::block::BlockHeader;
use ahda::printer::Printer;

use std::fs::File;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Read;
use std::io::Write;
use std::io::BufWriter;
use std::path::PathBuf;

use clap::Parser;

mod cli;

/// Initializes the logger with verbosity given in `log_max_level`.
fn init_log(log_max_level: usize) {
    stderrlog::new()
    .module(module_path!())
    .quiet(false)
    .verbosity(log_max_level)
    .timestamp(stderrlog::Timestamp::Off)
    .init()
    .unwrap();
}

fn main() {
    let cli = cli::Cli::parse();

    // Subcommands:
    match &cli.command {
        // Encode
        Some(cli::Commands::Encode {
            input_files,
            out_file,
            query_file,
            target_list,
            verbose,
        }) => {
            init_log(if *verbose { 2 } else { 1 });

            let mut reader = needletail::parse_fastx_file(query_file).expect("Valid fastX file");
            let mut queries: Vec<String> = Vec::new();
            while let Some(record) = reader.next() {
                let query_info = record.unwrap().id().iter().map(|x| *x as char).collect::<String>();
                let mut infos = query_info.split(' ');
                let query_name = infos.next().unwrap().to_string();
                queries.push(query_name);
            }

            let targets: Vec<String> = {
                let f = File::open(target_list).unwrap();
                let reader = BufReader::new(f);
                reader.lines().map(|line| line.unwrap()).collect::<Vec<String>>()
            };

            if input_files.len() > 1 && out_file.is_some() {
                panic!("-o/--output can only be used with a single input file");
            }

            if !input_files.is_empty() {
                for file in input_files {
                    let mut conn_in = File::open(file).unwrap();

                    let out_path = PathBuf::from(file.to_string_lossy().to_string() + ".ahda");
                    let f = File::create(out_path).unwrap();
                    let mut conn_out = BufWriter::new(f);

                    ahda::encode_from_std_read_to_std_write(&targets, &queries, &query_file.to_string_lossy(), &mut conn_in, &mut conn_out).unwrap();
                }
            } else {
                let mut conn_in = std::io::stdin();
                if out_file.is_some() {
                    let f = File::create(out_file.as_ref().unwrap()).unwrap();
                    let mut conn_out = BufWriter::new(f);
                    ahda::encode_from_std_read_to_std_write(&targets, &queries, &query_file.to_string_lossy(), &mut conn_in, &mut conn_out).unwrap();
                } else {
                    ahda::encode_from_std_read_to_std_write(&targets, &queries, &query_file.to_string_lossy(), &mut conn_in, &mut std::io::stdout()).unwrap();
                }
            }
        },

        // Decode
        Some(cli::Commands::Decode {
            input_files,
            format,
            write_to_stdout,
            verbose,
        }) => {
            init_log(if *verbose { 2 } else { 1 });

            input_files.iter().for_each(|file| {
                let out_name = file.file_stem().unwrap().to_string_lossy();
                let out_path = PathBuf::from(out_name.to_string());
                let f = File::create(out_path).unwrap();

                let mut conn_out = BufWriter::new(f);
                let mut conn_in = File::open(file).unwrap();

                ahda::decode_from_std_read_to_std_write(format, &mut conn_in, &mut conn_out).unwrap();
            });

        },

        // Cat
        Some(cli::Commands::Cat {
            input_file,
            verbose,
        }) => {
            init_log(if *verbose { 2 } else { 1 });
            todo!("Implement cat.")
        },

        // Set operations
        Some(cli::Commands::Set {
            input_files,
            format,
            union,
            intersection,
            diff,
            xor,
            verbose,
        }) => {
            init_log(if *verbose { 2 } else { 1 });
            assert!(input_files.len() > 1);

            // Read bitmap A from the first file
            let (header_a, flags_a, (mut bitmap_a, block_flags_a)) = {
                let mut conn_in = File::open(&input_files[0]).unwrap();
                let header = ahda::headers::file::read_file_header(&mut conn_in).unwrap();
                let mut flags_bytes: Vec<u8> = vec![0; header.flags_len as usize];
                conn_in.read_exact(&mut flags_bytes).unwrap();
                let flags = ahda::headers::file::decode_file_flags(&flags_bytes).unwrap();
                (header, flags, ahda::read_bitmap(&mut conn_in).unwrap())
            };

            // Read the remainning bitmaps and perform requested operation
            for file in input_files.iter().skip(1) {
                let mut conn_in = File::open(file).unwrap();
                let header_b = ahda::headers::file::read_file_header(&mut conn_in).unwrap();
                let mut flags_bytes: Vec<u8> = vec![0; header_b.flags_len as usize];
                conn_in.read_exact(&mut flags_bytes).unwrap();
                let flags_b = ahda::headers::file::decode_file_flags(&flags_bytes).unwrap();
                let (bitmap_b, _) = ahda::read_bitmap(&mut conn_in).unwrap();

                // Files must have same dimension and same targets
                assert_eq!(header_a.n_targets, header_b.n_targets);
                assert_eq!(header_a.n_queries, header_b.n_queries);
                assert_eq!(flags_a.target_names, flags_b.target_names);

                // Opcodes are mutually exclusive so this works
                if *union {
                    bitmap_a |= bitmap_b;
                } else if *intersection {
                    bitmap_a &= bitmap_b;
                } else if *diff {
                    bitmap_a -= bitmap_b;
                } else if *xor {
                    bitmap_a ^= bitmap_b;
                }
            }

            // TODO fix spaghetti
            let roaring_bytes: Vec<u8> = ahda::pack::serialize_roaring(&bitmap_a).unwrap();
            let block_header = BlockHeader{ num_records: header_a.n_queries, deflated_len: 0, block_len: 0, flags_len: 0, start_idx: 0, placeholder2: 0, placeholder3: 0 };
            let records = ahda::unpack::decode_from_roaring(&flags_a, &block_header, &block_flags_a, flags_a.target_names.len() as u32, &roaring_bytes).unwrap();

            let mut printer = match format.as_str() {
                "bifrost" => Printer::new_from_flags(&records, &flags_a, &ahda::Format::Bifrost),
                "fulgor" => Printer::new_with_format(&records, &ahda::Format::Fulgor),
                "metagraph" => Printer::new_with_format(&records, &ahda::Format::Metagraph),
                "sam" => Printer::new_with_format(&records, &ahda::Format::SAM),
                "themisto" => Printer::new_with_format(&records, &ahda::Format::Themisto),
                _ => panic!("Unrecognized format --format {}", format),
            };
            while let Some(line) = printer.next() {
                std::io::stdout().write_all(&line).unwrap();
            }
            std::io::stdout().flush().unwrap();

        },
        None => { todo!("Print help message.")},
    }
}
