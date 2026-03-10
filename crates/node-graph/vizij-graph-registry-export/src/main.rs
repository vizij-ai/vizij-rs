//! CLI for exporting the baked node registry as JSON.

use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;

use vizij_graph_core::schema;

fn print_usage() {
    eprintln!("Usage: vizij-graph-registry-export [--output <path>] [--pretty]");
}

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
