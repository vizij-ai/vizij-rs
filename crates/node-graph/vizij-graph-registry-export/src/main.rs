//! Export the node registry to JSON for tooling.
//!
//! Run with no arguments to print a compact JSON registry to stdout.
//! Use `--pretty` for pretty JSON or `--output <path>` to write to a file.

use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;

use vizij_graph_core::schema;

/// Internal helper for `print_usage`.
fn print_usage() {
    eprintln!("Usage: vizij-graph-registry-export [--output <path>] [--pretty]");
}

/// CLI entry point for exporting the node registry JSON.
///
/// # Errors
///
/// Returns an error when argument parsing fails, registry serialization fails,
/// or writing the output fails.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let mut output_path: Option<PathBuf> = None;
    let mut pretty = false;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--output" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--output flag expects a file path".to_string())?;
                output_path = Some(PathBuf::from(value));
            }
            "--pretty" => {
                pretty = true;
            }
            "--help" | "-h" => {
                print_usage();
                return Ok(());
            }
            unknown => {
                return Err(format!("Unknown argument '{unknown}'").into());
            }
        }
    }

    let registry = schema::registry();
    let json = if pretty {
        serde_json::to_string_pretty(&registry)?
    } else {
        serde_json::to_string(&registry)?
    };

    if let Some(path) = output_path {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, json)?;
    } else {
        let mut stdout = io::stdout().lock();
        stdout.write_all(json.as_bytes())?;
        stdout.flush()?;
    }

    Ok(())
}
