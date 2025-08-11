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

            let (sample_name, n_queries) = if let Some(file) = query_file {
                let mut reader = needletail::parse_fastx_file(file).expect("Valid fastX file");
                let mut count = 0;
                while let Some(record) = reader.next() {
                    let query_info = record.unwrap().id().iter().map(|x| *x as char).collect::<String>();
                    let mut infos = query_info.split(' ');
                    let query_name = infos.next().unwrap().to_string();

                    query_to_pos.insert(query_name, count);
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

            input_files.iter().for_each(|file| {
                let mut conn_in = File::open(file).unwrap();
                let mut records = ahda::parse(&mut conn_in).unwrap();

                let out_path = PathBuf::from(file.to_string_lossy().to_string() + ".ahda");
                let f = File::create(out_path).unwrap();
                let mut conn_out = BufWriter::new(f);

                if !records.is_empty() && records[0].query_id.is_none() {
                    records.iter_mut().for_each(|record| {
                        record.query_id = Some(*query_to_pos.get(&record.query_name.clone().unwrap()).unwrap() as u32);
                    })
                }

                ahda::encode(&records, &targets, &sample_name, n_queries, &mut conn_out).unwrap();
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
