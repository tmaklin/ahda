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
use std::io::IsTerminal;
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

type E = Box<dyn std::error::Error>;

struct FastxNameReader {
    reader: Box<dyn needletail::FastxReader>,
}

impl FastxNameReader {
    pub fn new(
        file: &PathBuf,
    ) -> Result<Self, E> {
        let reader = needletail::parse_fastx_file(file)?;
        Ok(Self{ reader })
    }
}

impl Iterator for FastxNameReader {
    type Item = Vec<u8>;

    fn next(
        &mut self,
    ) -> Option<Vec<u8>> {
        let record = self.reader.next()?;
        let query_info = record.unwrap();
        let end = query_info.id().iter().position(|x| x == &b' ');
        Some(query_info.id()[0..end.unwrap_or(query_info.id().len())].to_vec())
    }
}

fn main() -> Result<(),  Box<dyn std::error::Error>> {
    let cli = cli::Cli::parse();

    // Subcommands:
    match &cli.command {
        // Encode
        Some(cli::Commands::Encode {
            input_file,
            query_file,
            target_list,
            sample_name,
            stdout,
            force,
            keep,
            verbose,
        }) => {
            init_log(if *verbose { 2 } else { 1 });

            let mut targets = None;
            if let Some(target_list) = target_list {
                match File::open(target_list) {
                    Ok(f) => {
                        let reader = BufReader::new(f);
                        targets = Some(reader.split(b'\n').map(|x| x.unwrap()).collect::<Vec<Vec<u8>>>());
                    },
                    Err(e) => {
                        eprintln!("ahda: can't open input file `{}`: {}", target_list.to_string_lossy(), e);
                        return Err(Box::new(e))
                    },
                }
            }

            let queries: Option<FastxNameReader> = if let Some(query_file) = query_file {
                match FastxNameReader::new(query_file) {
                    Ok(reader) => Some(reader),
                    Err(e) => {
                        eprintln!("ahda: can't open input file `{}`: {}", query_file.to_string_lossy(), e);
                        return Err(e)
                    },
                }
            } else {
                None
            };

            let mut inputs: Vec<Box<dyn Read>> = Vec::new();
            let mut outputs: Vec<Box<dyn Write>> = Vec::new();
            let mut force_stdout: bool = false;
            if let Some(input_file) = input_file {
                match File::open(input_file) {
                    Ok(conn_in) => inputs.push(Box::new(conn_in)),
                    Err(e) => {
                        eprintln!("ahda: can't open input file `{}`: {}", input_file.to_string_lossy(), e);
                        return Err(Box::new(e))
                    },
                }

                let out_path = PathBuf::from(input_file.to_string_lossy().to_string() + ".ahda");

                if !*stdout {
                    match if *force { File::create(out_path.clone()) } else { File::create_new(out_path.clone()) } {
                        Ok(conn_out) => {
                            outputs.push(Box::new(conn_out));
                        },
                        Err(e) => {
                            eprintln!("ahda: can't create output file `{}`: {}", out_path.to_string_lossy(), e);
                            return Err(Box::new(e))
                        },
                    }
                }
            } else {
                inputs.push(Box::new(std::io::stdin()));
                if !*force  && std::io::stdout().is_terminal() {
                    eprintln!("ahda: refusing to write binary data to terminal, use `--force` to override");
                    return Ok(());
                } else {
                    force_stdout = true;
                }
            }

            if *stdout || force_stdout {
                outputs.push(Box::new(std::io::stdout()));
            }

            let conn_in = &mut inputs[0];
            let conn_out = &mut outputs[0];
            #[allow(clippy::manual_map)]
            let t_it = if let Some(t) = targets { Some(&mut t.into_iter()) } else { None };
            let ret = if let Some(mut q_it) = queries {
                let sample = if let Some(name) = sample_name { name.as_bytes().to_vec() } else { query_file.as_ref().unwrap().to_string_lossy().as_bytes().to_vec() };
                ahda::encode_from_read_to_write(t_it, Some(&mut q_it), &sample, conn_in, conn_out)
            } else {
                let sample = if let Some(name) = sample_name { name.as_bytes().to_vec() } else {
                    eprintln!("ahda: use `--name` to supply the sample name");
                    return Ok(())
                };
                ahda::encode_from_read_to_write(t_it, None::<&mut std::iter::Empty<Vec<u8>>>, &sample, conn_in, conn_out)
            };
            if let Err(e) = ret {
                eprintln!("ahda: can't encode input file `{}`: {}", input_file.as_ref().unwrap().to_string_lossy(), e);
                return Err(e)
            }

            if !*keep && !*stdout && input_file.is_some() {
                match std::fs::remove_file(input_file.as_ref().unwrap()) {
                    Ok(()) => (),
                    Err(e) => {
                        eprintln!("ahda: can't remove input file `{}`: {}", input_file.as_ref().unwrap().to_string_lossy(), e);
                        return Err(Box::new(e))
                    }
                }
            }

            Ok(())
        },

        // Decode
        Some(cli::Commands::Decode {
            input_file,
            format,
            stdout,
            force,
            keep,
            verbose,
        }) => {
            init_log(if *verbose { 2 } else { 1 });

            let mut conn_in: Vec<Box<dyn Read>> = Vec::new();
            let mut conn_out: Vec<Box<dyn Write>> = Vec::new();
            if let Some(file) = input_file {
                if *stdout {
                    conn_out.push(Box::new(std::io::stdout()));
                } else {
                    let out_name = file.file_stem().unwrap().to_string_lossy();
                    let out_path = PathBuf::from(out_name.to_string());

                    if !*stdout {
                        match if *force { File::create(out_path.clone()) } else { File::create_new(out_path.clone()) } {
                            Ok(out) => {
                                conn_out.push(Box::new(out));
                            },
                            Err(e) => {
                                eprintln!("ahda: can't create output file `{}`: {}", out_path.to_string_lossy(), e);
                                return Err(Box::new(e))
                            },
                        }
                    }

                }
                match File::open(file) {
                    Ok(conn) => conn_in.push(Box::new(conn)),
                    Err(e) => {
                        eprintln!("ahda: can't open input file `{}`: {}", file.to_string_lossy(), e);
                        return Err(Box::new(e))
                    },
                }
            } else {
                if !*force  && std::io::stdin().is_terminal() {
                    eprintln!("ahda: standard input is a terminal, use `--force` to ignore");
                    return Ok(());
                }
                conn_in.push(Box::new(std::io::stdin()));
                conn_out.push(Box::new(std::io::stdout()));
            }

            ahda::decode_from_read_to_write(format.clone().unwrap_or_default(), &mut conn_in[0], &mut conn_out[0]).unwrap();

            if !*keep && !*stdout && input_file.is_some() {
                match std::fs::remove_file(input_file.as_ref().unwrap()) {
                    Ok(()) => (),
                    Err(e) => {
                        eprintln!("ahda: can't remove input file `{}`: {}", input_file.as_ref().unwrap().to_string_lossy(), e);
                        return Err(Box::new(e))
                    }
                }
            }

            Ok(())
        },

        // // Cat
        // Some(cli::Commands::Cat {
        //     input_files,
        //     verbose,
        // }) => {
        //     todo!("Implement `ahda cat`");
        //     init_log(if *verbose { 2 } else { 1 });

        //     let mut inputs: Vec<Box<dyn Read>> = Vec::new();
        //     for file in input_files {
        //         let conn_in = File::open(file).unwrap();
        //         inputs.push(Box::new(conn_in));
        //     }
        //     let mut conn_out = std::io::stdout();

        //     ahda::concatenate_from_read_to_write(&mut inputs, &mut conn_out).unwrap();
        //     Ok(())
        // },

        // Convert
        Some(cli::Commands::Convert {
            input_file,
            query_file,
            target_list,
            output_file,
            format,
            sample_name,
            stdout,
            force,
            keep,
            verbose,
        }) => {
            init_log(if *verbose { 2 } else { 1 });

            let mut targets = None;
            if let Some(target_list) = target_list {
                match File::open(target_list) {
                    Ok(f) => {
                        let reader = BufReader::new(f);
                        targets = Some(reader.split(b'\n').map(|x| x.unwrap()).collect::<Vec<Vec<u8>>>());
                    },
                    Err(e) => {
                        eprintln!("ahda: can't open input file `{}`: {}", target_list.to_string_lossy(), e);
                        return Err(Box::new(e))
                    },
                }
            }

            let queries: Option<FastxNameReader> = if let Some(query_file) = query_file {
                match FastxNameReader::new(query_file) {
                    Ok(reader) => Some(reader),
                    Err(e) => {
                        eprintln!("ahda: can't open input file `{}`: {}", query_file.to_string_lossy(), e);
                        return Err(e)
                    },
                }
            } else {
                None
            };

            let mut inputs: Vec<Box<dyn Read>> = Vec::new();
            let mut outputs: Vec<Box<dyn Write>> = Vec::new();
            if let Some(input_file) = input_file {
                match File::open(input_file) {
                    Ok(conn_in) => inputs.push(Box::new(conn_in)),
                    Err(e) => {
                        eprintln!("ahda: can't open input file `{}`: {}", input_file.to_string_lossy(), e);
                        return Err(Box::new(e))
                    },
                }

                let out_path = if let Some(out) = output_file { out.clone() } else { PathBuf::from(input_file.to_string_lossy().to_string() + ".tmp") };

                if !*stdout {
                    match if *force { File::create(out_path.clone()) } else { File::create_new(out_path.clone()) } {
                        Ok(conn_out) => {
                            outputs.push(Box::new(conn_out));
                        },
                        Err(e) => {
                            eprintln!("ahda: can't create output file `{}`: {}", out_path.to_string_lossy(), e);
                            return Err(Box::new(e))
                        },
                    }
                }
            } else {
                if std::io::stdin().is_terminal() {
                    eprintln!("ahda: standard input is a terminal, ignoring");
                    return Ok(());
                }
                inputs.push(Box::new(std::io::stdin()));
            }

            if *stdout || (input_file.is_none() && output_file.is_none()) {
                outputs.push(Box::new(std::io::stdout()));
            }

            let conn_in = &mut inputs[0];
            let conn_out = &mut outputs[0];
            #[allow(clippy::manual_map)]
            let t_it = if let Some(t) = targets { Some(&mut t.into_iter()) } else { None };
            let ret = if let Some(mut q_it) = queries {
                let sample = if let Some(name) = sample_name { name.as_bytes().to_vec() } else { query_file.as_ref().unwrap().to_string_lossy().as_bytes().to_vec() };
                ahda::convert_from_read_to_write(t_it, Some(&mut q_it), &sample, format.as_ref().unwrap().clone(), conn_in, conn_out)
            } else {
                let sample = if let Some(name) = sample_name { name.as_bytes().to_vec() } else {
                    eprintln!("ahda: use `--name` to supply the sample name");
                    return Ok(())
                };
                ahda::convert_from_read_to_write(t_it, None::<&mut std::iter::Empty<Vec<u8>>>, &sample, format.as_ref().unwrap().clone(), conn_in, conn_out)
            };
            if let Err(e) = ret {
                eprintln!("ahda: can't convert input file `{}`: {}", input_file.as_ref().unwrap().to_string_lossy(), e);
                return Err(e)
            }

            if !*keep && !*stdout && input_file.is_some() && output_file.is_none() {
                let tmp_path = PathBuf::from(input_file.as_ref().unwrap().to_string_lossy().to_string() + ".tmp");
                match std::fs::rename(tmp_path.clone(), input_file.as_ref().unwrap()) {
                    Ok(()) => (),
                    Err(e) => {
                        eprintln!("ahda: can't rename output file `{}` to `{}`: {}", input_file.as_ref().unwrap().to_string_lossy(), tmp_path.to_string_lossy(), e);
                        return Err(Box::new(e))
                    }
                }
            }

            Ok(())
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

            let block_header = BlockHeader{ num_records: header_a.n_queries, bitmap_type: 0, metadata_compression: 0, block_len: 0, flags_len: 0, fields_present: 0, placeholder1: 0, placeholder2: 0, placeholder3: 0 };
            let mut iter = bitmap_a.iter();
            let mut decoder = ahda::decoder::bitmap_decoder::BitmapDecoder::new(&mut iter, header_a.clone(), flags_a.clone(), block_header, block_flags_a);
            let printer = Printer::new_from_header_and_flags(&mut decoder, header_a.clone(), flags_a.clone(), format.as_ref().unwrap().clone());
            for line in printer {
                std::io::stdout().write_all(&line).unwrap();
            }
            std::io::stdout().flush().unwrap();
            Ok(())

        },
        None => { eprintln!("ahda: Try 'ahda --help' for more information."); Ok(()) },
    }
}
