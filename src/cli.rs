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
        #[arg(group = "input", required = false, help = "Input file(s)")]
        input_files: Vec<PathBuf>,

        // Output file path
        #[arg(short = 'o', long = "output", required = false)]
        out_file: Option<PathBuf>,

        // FastX file used to generate the alignment
        #[arg(short = 'q', long = "query", required = true)]
        query_file: PathBuf,

        // File listing target sequence names in the order they appear in the index
        #[arg(long = "targets", required = true)]
        target_list: PathBuf,

        // Verbosity
        #[arg(long = "verbose", default_value_t = false)]
        verbose: bool,
    },

    // Decode .ahda format
    Decode {
        // Input file
        #[arg(group = "input", required = true, help = "Input file(s)")]
        input_files: Vec<PathBuf>,

        // Output format, defaults to Themisto
        #[arg(long = "format", default_value = "themisto")]
        format: String,

        // Verbosity
        #[arg(short = 'c', long = "stdout", default_value_t = false)]
        write_to_stdout: bool,

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

    // Set operations on .ahda files
    Set {
        // Input file
        #[arg(group = "input", required = true, help = "Input files")]
        input_files: Vec<PathBuf>,

        // Output format, defaults to Themisto
        #[arg(long = "format", default_value = "themisto")]
        format: String,

        // Operations
        // // Union
        #[arg(short = 'u', long = "union", group = "op", required = true, help = "Union (A or B)")]
        union: bool,
        // // Intersection
        #[arg(short = 'i', long = "intersection", group = "op", required = true, help = "Intersection (A and B)")]
        intersection: bool,
        // // Diff
        #[arg(short = 'd', long = "diff", group = "op", required = true, help = r"Difference (A \ B)")]
        diff: bool,
        // // Symmetric difference (XOR)
        #[arg(short = 'x', long = "xor", group = "op", required = true, help = "Symmetric difference (A xor B)")]
        xor: bool,

        // Verbosity
        #[arg(long = "verbose", default_value_t = false)]
        verbose: bool,
    },
}
