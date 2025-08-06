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
            verbose,
        }) => {
            init_log(if *verbose { 2 } else { 1 });

            input_files.iter().for_each(|file| {
                let mut conn_in = File::open(file).unwrap();
                let records = ahda::parse(*n_targets, &mut conn_in);

                let out_path = PathBuf::from(file.to_string_lossy().to_string() + ".ahda");
                let f = File::create(out_path).unwrap();
                let mut conn_out = BufWriter::new(f);

                let file_header = ahda::format::encode_file_header(0,0,0,0).unwrap();
                let _ = conn_out.write_all(&file_header);

                ahda::encode(&records, &mut conn_out).unwrap();
            });

        },

        // Decode
        Some(cli::Commands::Decode {
            input_files,
            n_targets,
            verbose,
        }) => {
            init_log(if *verbose { 2 } else { 1 });

            input_files.iter().for_each(|file| {
                let mut conn_in = File::open(file).unwrap();
                let header = ahda::format::read_file_header(&mut conn_in).unwrap();
                let records = ahda::decode(&header, &mut conn_in).unwrap();
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
