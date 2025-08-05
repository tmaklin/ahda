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
use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(version)]
#[command(propagate_version = true)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    // Encode pseudoalignment data in .ahda format
    Encode {
        // Input fasta or fastq sequence file(s)
        #[arg(group = "input", required = true, help = "Input file(s)")]
        input_files: Vec<PathBuf>,

        // Verbosity
        #[arg(long = "n-targets", required = true)]
        n_targets: usize,

        // Verbosity
        #[arg(long = "verbose", default_value_t = false)]
        verbose: bool,
    },

    // Decode .ahda format
    Decode {
        // Input file
        #[arg(group = "input", required = true, help = "Input file")]
        input_file: PathBuf,

        // Verbosity
        #[arg(long = "verbose", default_value_t = false)]
        verbose: bool,
    },

    // Convert between supported formats
    Cat {
        // Input file
        #[arg(group = "input", required = true, help = "Input file")]
        input_file: PathBuf,

        // Verbosity
        #[arg(long = "verbose", default_value_t = false)]
        verbose: bool,
    },
}
