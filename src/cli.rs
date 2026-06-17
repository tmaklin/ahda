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
    #[command(name = "encode", about = "Compress plain text data")]
    Encode {
        // Input fasta or fastq sequence file(s)
        #[arg(group = "input", required = false, help = "Input file")]
        input_file: Option<PathBuf>,

        // FastX file used to generate the alignment
        #[arg(short = 'q', long = "query", help_heading = "Inputs", help = "Query .fastX file")]
        query_file: Option<PathBuf>,

        // File listing target sequence names in the order they appear in the index
        #[arg(short = 't', long = "targets", help_heading = "Inputs", help = "File listing target sequence names")]
        target_list: Option<PathBuf>,

        // Sample name
        #[arg(short = 'n', long = "name", help_heading = "Inputs", help = "Sample name (default: .fastX file path)")]
        sample_name: Option<String>,

        // Override input format detection
        #[arg(short = 'F', long = "format", help_heading = "Inputs", help = "Force input format for plain text parser")]
        input_format: Option<ahda::Format>,

        // Write to stdout
        #[arg(short = 'c', long = "stdout", default_value_t = false, help = "Write to stdout, keep original file")]
        stdout: bool,

        // Overwrite output file
        #[arg(short = 'f', long = "force", default_value_t = false, help = "Force overwriting")]
        force: bool,

        // Overwrite query names
        #[arg(long = "rename", default_value_t = false, help = "Overwrite query names with `sample_name`.`query_id`")]
        rename: bool,

        // Keep original file
        #[arg(short = 'k', long = "keep", default_value_t = false, help = "Don't delete input file after finishing")]
        keep: bool,

        // Verbosity
        #[arg(short = 'v', long = "verbose", default_value_t = false, help = "Print extra information")]
        verbose: bool,
    },

    // Decode .ahda format
    #[command(name = "decode", about = "Decompress binary data")]
    Decode {
        // Input file
        #[arg(group = "input", required = false, help = "Input file(s)")]
        input_file: Option<PathBuf>,

        // Output format, defaults to Themisto
        #[arg(short = 'F', long = "format", required = false, help = "Output plain text format")]
        format: Option<ahda::Format>,

        // Write to stdout
        #[arg(short = 'c', long = "stdout", default_value_t = false, help = "Write to stdout, keep original file")]
        stdout: bool,

        // Overwrite output file
        #[arg(short = 'f', long = "force", default_value_t = false, help = "Force overwriting")]
        force: bool,

        // Keep original file
        #[arg(short = 'k', long = "keep", default_value_t = false, help = "Don't delete input file after finishing")]
        keep: bool,

        // Verbosity
        #[arg(short = 'v', long = "verbose", default_value_t = false, help = "Print extra information")]
        verbose: bool,
    },

    // Convert plaintext to another plaintext format
    #[command(name = "convert", about = "Convert between plain text formats")]
    Convert {
        // Input fasta or fastq sequence file(s)
        #[arg(group = "input", required = false, help = "Input file")]
        input_file: Option<PathBuf>,

        // FastX file used to generate the alignment
        #[arg(short = 'q', long = "query", help_heading = "Inputs", help = "Query .fastX file")]
        query_file: Option<PathBuf>,

        // File listing target sequence names in the order they appear in the index
        #[arg(short = 't', long = "targets", help_heading = "Inputs", help = "File listing target sequence names")]
        target_list: Option<PathBuf>,

        // Output file name
        #[arg(short = 'o', long = "output", help_heading = "Outputs", help = "Output to file, keep original file")]
        output_file: Option<PathBuf>,

        // Output format, defaults to Themisto
        #[arg(short = 'F', long = "format", required = false, help_heading = "Outputs", help = "Output plain text format")]
        format: Option<ahda::Format>,

        // Sample name
        #[arg(short = 'n', long = "name", help_heading = "Inputs", help = "Sample name (default: .fastX file path)")]
        sample_name: Option<String>,

        // Write to stdout
        #[arg(short = 'c', long = "stdout", default_value_t = false, help = "Write to stdout, keep original file")]
        stdout: bool,

        // Overwrite output file
        #[arg(short = 'f', long = "force", default_value_t = false, help = "Force overwriting")]
        force: bool,

        // Keep original file
        #[arg(short = 'k', long = "keep", default_value_t = false, help = "Don't delete input file after finishing")]
        keep: bool,

        // Verbosity
        #[arg(short = 'v', long = "verbose", default_value_t = false, help = "Print extra information")]
        verbose: bool,
    },

    // Concatenate encoded data
    #[command(name = "cat", about = "Concatenate binary data")]
    Cat {
        // Input files
        #[arg(group = "input", required = true, help = "Input file(s)")]
        input_files: Vec<PathBuf>,

        // Output file name
        #[arg(short = 'o', long = "output", help_heading = "Outputs", help = "Output to file")]
        output_file: Option<PathBuf>,

        // Write to stdout
        #[arg(short = 'c', long = "stdout", default_value_t = false, help = "Write to stdout, keep original file")]
        stdout: bool,

        // Overwrite output file
        #[arg(short = 'f', long = "force", default_value_t = false, help = "Force overwriting")]
        force: bool,

        // Verbosity
        #[arg(short = 'v', long = "verbose", default_value_t = false, help = "Print extra information")]
        verbose: bool,
    },

    // Set operations on .ahda files
    #[command(name = "set", about = "Set operations on binary data")]
    Set {
        // Input files
        #[arg(group = "input", required = true, help = "Input file(s)")]
        input_files: Vec<PathBuf>,

        // Output file name
        #[arg(short = 'o', long = "output", help_heading = "Outputs", help = "Output to file")]
        output_file: Option<PathBuf>,

        // Output format, defaults to Themisto
        #[arg(short = 'F', long = "format", help_heading = "Outputs", required = false, help = "Output plain text format")]
        format: Option<ahda::Format>,

        // Merge operation
        #[arg(short = 'm', long = "mode", default_value = "union", help = "Merge operation")]
        operation: Option<ahda::MergeOp>,

        // Write to stdout
        #[arg(short = 'c', long = "stdout", default_value_t = false, help = "Write to stdout, keep original file")]
        stdout: bool,

        // Overwrite output file
        #[arg(short = 'f', long = "force", default_value_t = false, help = "Force overwriting")]
        force: bool,

        // Verbosity
        #[arg(short = 'v', long = "verbose", default_value_t = false, help = "Print extra information")]
        verbose: bool,
    },
}
