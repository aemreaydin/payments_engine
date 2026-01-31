use std::fs::File;
use std::io::{self, BufReader};
use std::path::PathBuf;
use std::process;

use clap::Parser;

use payments_engine::io::{process_csv, write_accounts};

#[derive(Parser)]
struct Args {
    file: PathBuf,
}

fn main() {
    let args = Args::parse();

    let file = File::open(&args.file).unwrap_or_else(|e| {
        eprintln!("Error opening {}: {e}", args.file.display());
        process::exit(1);
    });

    let reader = BufReader::new(file);

    let engine = process_csv(reader).unwrap_or_else(|e| {
        eprintln!("Error processing CSV: {e}");
        process::exit(1);
    });

    let stdout = io::stdout();
    if let Err(e) = write_accounts(stdout.lock(), &engine) {
        eprintln!("Error writing output: {e}");
        process::exit(1);
    }
}
