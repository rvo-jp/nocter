use std::ffi::OsString;
use std::path::Path;

use nocter_content_integrity::{TreeHashOptions, sha256_file, sha256_regular_tree};

fn main() {
    let arguments = std::env::args_os().skip(1).collect::<Vec<_>>();
    if let Err(error) = run(&arguments) {
        eprintln!("content integrity error: {error}");
        std::process::exit(2);
    }
}

fn run(arguments: &[OsString]) -> Result<(), Box<dyn std::error::Error>> {
    let [operation, path] = arguments else {
        return Err("usage: nocter-content-integrity <file|tree> PATH".into());
    };
    let digest = match operation.to_str() {
        Some("file") => sha256_file(Path::new(path))?,
        Some("tree") => sha256_regular_tree(Path::new(path), TreeHashOptions::complete())?,
        _ => return Err("operation must be `file` or `tree`".into()),
    };
    println!("{digest}");
    Ok(())
}
