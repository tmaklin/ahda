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

            let mut inputs: Vec<Box<dyn Read>> = Vec::new();
            let mut outputs: Vec<Box<dyn Write>> = Vec::new();
            if !input_files.is_empty() {
                for file in input_files {
                    let conn_in = File::open(file).unwrap();
                    inputs.push(Box::new(conn_in));

                    let out_path = PathBuf::from(file.to_string_lossy().to_string() + ".ahda");
                    let f = File::create(out_path).unwrap();
                    let conn_out = BufWriter::new(f);
                    outputs.push(Box::new(conn_out));

                }
            } else {
                let conn_in = std::io::stdin();
                inputs.push(Box::new(conn_in));

                let conn_out = std::io::stdout();
                outputs.push(Box::new(conn_out));
            }

            inputs.iter_mut().zip(outputs.iter_mut()).for_each(|(conn_in, conn_out)| {
                ahda::encode_from_read_to_write(&targets, &queries, &query_file.to_string_lossy(), &mut *conn_in, &mut *conn_out).unwrap();
            })
        },

        // Decode
        Some(cli::Commands::Decode {
            input_files,
            format,
            verbose,
        }) => {
            init_log(if *verbose { 2 } else { 1 });

            input_files.iter().for_each(|file| {
                let out_name = file.file_stem().unwrap().to_string_lossy();
                let out_path = PathBuf::from(out_name.to_string());
                let f = File::create(out_path).unwrap();

                let mut conn_out = BufWriter::new(f);
                let mut conn_in = File::open(file).unwrap();

                ahda::decode_from_read_to_write(format.clone().unwrap_or_default(), &mut conn_in, &mut conn_out).unwrap();
            });

        },

        // Cat
        Some(cli::Commands::Cat {
            input_files,
            verbose,
        }) => {
            init_log(if *verbose { 2 } else { 1 });

            let mut inputs: Vec<Box<dyn Read>> = Vec::new();
            for file in input_files {
                let conn_in = File::open(file).unwrap();
                inputs.push(Box::new(conn_in));
            }
            let mut conn_out = std::io::stdout();

            ahda::concatenate_from_read_to_write(&mut inputs, &mut conn_out).unwrap();
        },

        // Convert
        Some(cli::Commands::Convert {
            input_file,
            query_file,
            target_list,
            format,
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

            let mut conn_in = File::open(input_file).unwrap();
            let mut conn_out = std::io::stdout();

            let sample_name = query_file.file_stem().unwrap().to_string_lossy();
            ahda::convert_from_read_to_write(&targets, &queries, &sample_name, format.as_ref().unwrap().clone(), &mut conn_in, &mut conn_out).unwrap();
        },

        // Set operations
        Some(cli::Commands::Set {
            input_files,
            format,
            operation,
            verbose,
        }) => {
            init_log(if *verbose { 2 } else { 1 });
            assert!(input_files.len() > 1);

            // Read bitmap A from the first file
            let mut conn_in = File::open(&input_files[0]).unwrap();
            let (mut bitmap_a, header_a, flags_a, block_flags_a) = ahda::decode_from_read_to_roaring(&mut conn_in).unwrap();

            // Read the remainning bitmaps and perform requested operation
            for file in input_files.iter().skip(1) {
                let mut conn_in = File::open(file).unwrap();
                ahda::decode_from_read_into_roaring(&mut conn_in, operation.as_ref().unwrap(), &mut bitmap_a).unwrap();
            }

            let block_header = BlockHeader{ num_records: header_a.n_queries, deflated_len: 0, block_len: 0, flags_len: 0, start_idx: 0, placeholder2: 0, placeholder3: 0 };
            let mut iter = bitmap_a.iter().map(|x| x as u64);
            let mut decoder = ahda::decoder::bitmap::BitmapDecoder::new(&mut iter, header_a.clone(), flags_a.clone(), block_header, block_flags_a);
            let printer = Printer::new_from_header_and_flags(&mut decoder, header_a.clone(), flags_a.clone(), format.as_ref().unwrap().clone());
            for line in printer {
                std::io::stdout().write_all(&line).unwrap();
            }
            std::io::stdout().flush().unwrap();

        },
        None => { eprintln!("ahda: Try 'ahda --help' for more information.") },
    }
}
