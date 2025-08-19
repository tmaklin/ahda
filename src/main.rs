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
use ahda::headers::file::FileHeader;
use ahda::printer::Printer;

use std::collections::HashMap;
use std::fs::File;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Read;
use std::io::Write;
use std::io::BufWriter;
use std::path::PathBuf;

use clap::Parser;

mod cli;

type E = Box<dyn std::error::Error>;

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

fn encode_file<R: Read, W: Write>(
    targets: &[String],
    query_name: &str,
    query_to_pos: &HashMap<String, usize>,
    pos_to_query: &HashMap<usize, String>,
    conn_in: &mut R,
    conn_out: &mut W,
) -> Result<(), E> {
    let n_targets = targets.len();
    let n_queries = query_to_pos.len();

    // Adjust block size to fit within 32-bit address space
    let block_size = ((u32::MAX as u64) / n_targets as u64).min(65537_u64) as usize;
    assert!(block_size > 1);
    let block_size = block_size - 1;

    let flags_bytes = ahda::headers::file::encode_file_flags(targets, query_name)?;

    let mut reader = ahda::parser::Parser::new(conn_in)?;

    // TODO fix
    let file_header = FileHeader{ n_targets: n_targets as u32, n_queries: n_queries as u32, flags_len: flags_bytes.len() as u32, format: 1_u16, ph2: 0, ph3: 0, ph4: 0 };
    let file_header_bytes = ahda::headers::file::encode_file_header(n_targets as u32, n_queries as u32, flags_bytes.len() as u32, 1, 0,0,0)?;

    conn_out.write_all(&file_header_bytes)?;
    conn_out.write_all(&flags_bytes)?;

    let mut records: Vec<PseudoAln> = Vec::new();
    while let Some(record) = reader.next() {
        records.push(record);
        if records.len() > block_size {
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

            records.sort_by_key(|x| x.query_id.unwrap());

            ahda::encode_block(&file_header, &records, conn_out)?;
            records.clear();
        }
    }
    if !records.is_empty() {
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
        ahda::encode_block(&file_header, &records, conn_out)?;
    }
    Ok(())
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

            let mut query_to_pos: HashMap<String, usize> = HashMap::new();
            let mut pos_to_query: HashMap<usize, String> = HashMap::new();

            let mut reader = needletail::parse_fastx_file(query_file).expect("Valid fastX file");
            let mut count = 0;
            while let Some(record) = reader.next() {
                let query_info = record.unwrap().id().iter().map(|x| *x as char).collect::<String>();
                let mut infos = query_info.split(' ');
                let query_name = infos.next().unwrap().to_string();

                query_to_pos.insert(query_name.clone(), count);
                pos_to_query.insert(count, query_name);
                count += 1;
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

                    encode_file(&targets, &query_file.to_string_lossy(), &query_to_pos, &pos_to_query, &mut conn_in, &mut conn_out).unwrap();
                }
            } else {
                let mut conn_in = std::io::stdin();
                if out_file.is_some() {
                    let f = File::create(out_file.as_ref().unwrap()).unwrap();
                    let mut conn_out = BufWriter::new(f);
                    encode_file(&targets, &query_file.to_string_lossy(), &query_to_pos, &pos_to_query, &mut conn_in, &mut conn_out).unwrap();
                } else {
                    encode_file(&targets, &query_file.to_string_lossy(), &query_to_pos, &pos_to_query, &mut conn_in, &mut std::io::stdout()).unwrap();
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
                let mut conn_in = File::open(file).unwrap();

                let file_header = ahda::headers::file::read_file_header(&mut conn_in).unwrap();

                let mut dump: Vec<u8> = vec![0; file_header.flags_len as usize];
                let _ = conn_in.read_exact(&mut dump);

                let out_name = file.file_stem().unwrap().to_string_lossy();
                let out_path = PathBuf::from(out_name.to_string());
                let f = File::create(out_path).unwrap();

                let mut conn_out = BufWriter::new(f);

                while let Ok(records) = ahda::decode_block_from_std_read(&file_header, &mut conn_in) {
                    let mut printer = match format.as_str() {
                        "themisto" => Printer::new_with_format(&records, &ahda::Format::Themisto),
                        "fulgor" => Printer::new_with_format(&records, &ahda::Format::Fulgor),
                        _ => panic!("Unrecognized format --format {}", format),
                    };
                    while let Some(line) = printer.next() {
                        if *write_to_stdout {
                            std::io::stdout().write_all(&line).unwrap();
                        } else {
                            conn_out.write_all(&line).unwrap();
                        }
                    }
                    conn_out.flush().unwrap();
                }
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
