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
use ahda::Format;
use ahda::PseudoAln;
use ahda::printer::Printer;

use std::collections::HashMap;
use std::fs::File;
use std::io::BufRead;
use std::io::BufReader;
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
            n_targets,
            query_file,
            target_list,
            verbose,
        }) => {
            init_log(if *verbose { 2 } else { 1 });

            let mut query_to_pos: HashMap<String, usize> = HashMap::new();
            let mut pos_to_query: HashMap<usize, String> = HashMap::new();

            let (sample_name, n_queries) = if let Some(file) = query_file {
                let mut reader = needletail::parse_fastx_file(file).expect("Valid fastX file");
                let mut count = 0;
                while let Some(record) = reader.next() {
                    let query_info = record.unwrap().id().iter().map(|x| *x as char).collect::<String>();
                    let mut infos = query_info.split(' ');
                    let query_name = infos.next().unwrap().to_string();

                    query_to_pos.insert(query_name.clone(), count);
                    pos_to_query.insert(count, query_name);
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
                vec![String::new(); *n_targets]
            };

            let block_size = 65536;

            input_files.iter().for_each(|file| {
                let mut conn_in = File::open(file).unwrap();
                let mut reader = if let Ok(parser) = ahda::parser::Parser::new(&mut conn_in) {
                    parser
                } else {
                    panic!("Unknown input format.");
                };

                let out_path = PathBuf::from(file.to_string_lossy().to_string() + ".ahda");
                let f = File::create(out_path).unwrap();
                let mut conn_out = BufWriter::new(f);

                let flags_bytes = ahda::headers::file::encode_file_flags(&targets, &query_file.as_ref().unwrap().to_string_lossy()).unwrap();
                let file_header = ahda::headers::file::encode_file_header(*n_targets as u32, n_queries as u32, flags_bytes.len() as u32, 1, 0,0,0).unwrap();

                conn_out.write_all(&file_header).unwrap();
                conn_out.write_all(&flags_bytes).unwrap();

                let mut records: Vec<PseudoAln> = Vec::new();
                while let Some(record) = reader.next() {
                    records.push(record);
                    if records.len() > block_size {
                        if query_file.is_some() || (!records.is_empty() && records[0].query_id.is_none()) {
                            match reader.format {
                                Format::Fulgor => {
                                    records.iter_mut().for_each(|record| {
                                        record.query_id = Some(*query_to_pos.get(&record.query_name.clone().unwrap()).unwrap() as u32);
                                    });
                                },
                                Format::Themisto => {
                                    records.iter_mut().for_each(|record| {
                                        record.query_name = Some(pos_to_query.get(&(record.query_id.unwrap() as usize)).unwrap().clone());
                                    });
                                },
                                _ => todo!("Implement remaining formats"),
                            }
                        }

                        records.sort_by_key(|x| x.query_id.unwrap());

                        ahda::encode_block(&records, *n_targets, &mut conn_out).unwrap();
                        records.clear();
                    }
                }
            });

        },

        // Decode
        Some(cli::Commands::Decode {
            input_files,
            format,
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

                let mut printer = match format.as_str() {
                    "themisto" => Printer::new_with_format(&records, &ahda::Format::Themisto),
                    "fulgor" => Printer::new_with_format(&records, &ahda::Format::Fulgor),
                    _ => panic!("Unrecognized format --format {}", format),
                };

                while let Some(line) = printer.next() {
                    conn_out.write_all(&line).unwrap();
                }
                conn_out.flush().unwrap();
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
