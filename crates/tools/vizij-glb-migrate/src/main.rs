use std::path::{Path, PathBuf};
use std::process::ExitCode;

use vizij_glb_migrate::glb::Glb;
use vizij_glb_migrate::migrate::migrate_gltf_json;

const USAGE: &str = "\
Rewrite the Value-bearing JSON embedded in Vizij face-bundle .glb files
to canonical arora Value serde. The binary chunk is preserved
byte-for-byte; see the crate README for the exact documents touched.

Usage: vizij-glb-migrate [OPTIONS] <input.glb>...

Options:
  -o, --output <path>  Write the migrated file to <path> instead of
                       rewriting the input (single input only).
      --dry-run        Report what would change without writing anything.
      --check          Like --dry-run, but exit with status 1 when any
                       input would change (for CI).
  -h, --help           Print this help.

Without --output, inputs are rewritten in place; the original bytes are
first saved next to the input as <input>.bak (an existing backup is
overwritten). Files already in canonical form are left untouched.

Exit status: 0 success, 1 --check found files needing migration, 2 error.
";

struct Options {
    inputs: Vec<PathBuf>,
    output: Option<PathBuf>,
    dry_run: bool,
    check: bool,
}

/// `Ok(None)` means help was printed and the process should exit cleanly.
fn parse_args(args: &[String]) -> Result<Option<Options>, String> {
    let mut inputs = Vec::new();
    let mut output = None;
    let mut dry_run = false;
    let mut check = false;
    let mut positional_only = false;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if positional_only || !arg.starts_with('-') {
            inputs.push(PathBuf::from(arg));
            continue;
        }
        match arg.as_str() {
            "--" => positional_only = true,
            "-h" | "--help" => {
                print!("{USAGE}");
                return Ok(None);
            }
            "-o" | "--output" => {
                let value = iter
                    .next()
                    .ok_or_else(|| format!("{arg} requires a path"))?;
                output = Some(PathBuf::from(value));
            }
            "--dry-run" => dry_run = true,
            "--check" => check = true,
            other => {
                if let Some(value) = other.strip_prefix("--output=") {
                    output = Some(PathBuf::from(value));
                } else {
                    return Err(format!("unknown option '{other}'"));
                }
            }
        }
    }
    if inputs.is_empty() {
        return Err("no input files given".to_string());
    }
    if output.is_some() && inputs.len() > 1 {
        return Err("--output requires exactly one input".to_string());
    }
    Ok(Some(Options {
        inputs,
        output,
        dry_run,
        check,
    }))
}

enum Outcome {
    Unchanged,
    WouldChange,
    Rewritten,
}

fn backup_path(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(".bak");
    PathBuf::from(name)
}

fn process_file(path: &Path, options: &Options) -> Result<Outcome, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("read: {e}"))?;
    let mut glb = Glb::parse(&bytes).map_err(|e| e.to_string())?;
    let mut root: serde_json::Value =
        serde_json::from_slice(&glb.json).map_err(|e| format!("JSON chunk: {e}"))?;
    let report = migrate_gltf_json(&mut root).map_err(|e| e.to_string())?;
    for warning in &report.warnings {
        eprintln!("{}: warning: {}", path.display(), warning);
    }
    if !report.changed() {
        println!("{}: unchanged", path.display());
        return Ok(Outcome::Unchanged);
    }
    if options.dry_run || options.check {
        println!("{}: would update {}", path.display(), report.summary());
        return Ok(Outcome::WouldChange);
    }
    glb.json = serde_json::to_vec(&root).map_err(|e| format!("serialize JSON chunk: {e}"))?;
    let out_bytes = glb.to_bytes();
    match &options.output {
        Some(output) => {
            std::fs::write(output, &out_bytes)
                .map_err(|e| format!("write {}: {e}", output.display()))?;
            println!(
                "{}: updated {} -> {}",
                path.display(),
                report.summary(),
                output.display()
            );
        }
        None => {
            let backup = backup_path(path);
            std::fs::write(&backup, &bytes)
                .map_err(|e| format!("write backup {}: {e}", backup.display()))?;
            std::fs::write(path, &out_bytes).map_err(|e| format!("write: {e}"))?;
            println!(
                "{}: updated {} (backup: {})",
                path.display(),
                report.summary(),
                backup.display()
            );
        }
    }
    Ok(Outcome::Rewritten)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let options = match parse_args(&args) {
        Ok(Some(options)) => options,
        Ok(None) => return ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("error: {message}");
            eprintln!("Run with --help for usage.");
            return ExitCode::from(2);
        }
    };
    let mut failed = false;
    let mut needs_migration = false;
    for input in &options.inputs {
        match process_file(input, &options) {
            Ok(Outcome::WouldChange) => needs_migration = true,
            Ok(Outcome::Unchanged | Outcome::Rewritten) => {}
            Err(message) => {
                eprintln!("{}: error: {message}", input.display());
                failed = true;
            }
        }
    }
    if failed {
        ExitCode::from(2)
    } else if options.check && needs_migration {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}
