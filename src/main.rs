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
use std::fs::File;
use std::io::BufRead;
use std::io::BufReader;
use std::io::BufWriter;
use std::io::Write;
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
            n_targets,
            query_file,
            target_list,
            verbose,
        }) => {
            init_log(if *verbose { 2 } else { 1 });

            let (query_name, n_queries) = if let Some(file) = query_file {
                let mut reader = needletail::parse_fastx_file(file).expect("Valid fastX file");
                let mut count = 0;
                while reader.next().is_some() {
                    count += 1;
                }
                let mut file_name: PathBuf = PathBuf::from(file.file_name().unwrap());
                while let Some(stripped) = file_name.file_stem() {
                    let is_same = file_name == stripped;
                    file_name = PathBuf::from(stripped);
                    if is_same {
                        break;
                    }
                };
                (file_name.to_str().unwrap().to_owned(), count)
            } else {
                (String::new(), 0)
            };

            let targets: Vec<String> = if let Some(file) = target_list {
                let f = File::open(file).unwrap();
                let reader = BufReader::new(f);
                reader.lines().map(|line| line.unwrap()).collect::<Vec<String>>()
            } else {
                Vec::new()
            };

            input_files.iter().for_each(|file| {
                let mut conn_in = File::open(file).unwrap();
                let records = ahda::parse(*n_targets, &mut conn_in);

                let out_path = PathBuf::from(file.to_string_lossy().to_string() + ".ahda");
                let f = File::create(out_path).unwrap();
                let mut conn_out = BufWriter::new(f);

                let flags_bytes = ahda::headers::file::encode_file_flags(&targets, &query_name).unwrap();
                let file_header = ahda::headers::file::encode_file_header(*n_targets as u32, n_queries as u32, flags_bytes.len() as u32, 1, 0,0,0).unwrap();

                let _ = conn_out.write_all(&file_header);
                let _ = conn_out.write_all(&flags_bytes);

                ahda::encode(&records, &mut conn_out).unwrap();
                conn_out.flush().unwrap();
            });

        },

        // Decode
        Some(cli::Commands::Decode {
            input_files,
            verbose,
        }) => {
            init_log(if *verbose { 2 } else { 1 });

            input_files.iter().for_each(|file| {
                let mut conn_in = File::open(file).unwrap();
                let records = ahda::decode(&mut conn_in).unwrap();

                let out_name = file.file_stem().unwrap().to_string_lossy();
                let out_path = PathBuf::from(out_name.to_string());
                let f = File::create(out_path).unwrap();

                let mut conn_out = BufWriter::new(f);
                records.iter().for_each(|record| {
                    let mut line = record.read_id.to_string();
                    record.ones.iter().enumerate().for_each(|(idx, is_set)| {
                        if *is_set {
                            line += " ";
                            line += &idx.to_string();
                        }
                    });
                    line += "\n";
                    let _ = conn_out.write_all(line.as_bytes());
                });
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
        None => { todo!("Print help message.")},
    }
}
