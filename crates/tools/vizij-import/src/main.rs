use std::path::{Path, PathBuf};
use std::process::ExitCode;

use vizij_import::morph_triangles::{Outcome as PartOutcome, Part};
use vizij_import::optimize_morph_triangles;

const USAGE: &str = "\
Run optional import passes over face .glb files. Each pass rewrites mesh
data only; the JSON chunk is preserved byte-for-byte.

Usage: vizij-import <PASS>... [OPTIONS] <input.glb>...

Passes (at least one):
      --morph-triangles  Re-triangulate each part modelled as one large
                         polygon so its triangles stay unfolded across its
                         morph targets, and report the shapes that still
                         fold.

Options:
  -o, --output <path>  Write the result to <path> instead of rewriting the
                       input (single input only).
      --dry-run        Report what would change without writing anything.
      --check          Like --dry-run, but exit with status 1 when any
                       input would change (for CI).
  -h, --help           Print this help.

Without --output, inputs are rewritten in place; the original bytes are
first saved next to the input as <input>.bak (an existing backup is
overwritten). Files the passes would not change are left untouched.

Exit status: 0 success, 1 --check found files that would change, 2 error.
";

/// A shape is listed when its reversed area, before or after, exceeds this
/// fraction of its area; below it the difference is not visible.
const LISTED: f64 = 0.001;

/// A shape that still has more than this fraction reversed after the pass
/// is flagged: no triangulation of its vertices follows its outline, so the
/// part needs remodelling.
const STILL_FOLDS: f64 = 0.01;

struct Options {
    inputs: Vec<PathBuf>,
    output: Option<PathBuf>,
    dry_run: bool,
    check: bool,
    morph_triangles: bool,
}

/// `Ok(None)` means help was printed and the process should exit cleanly.
fn parse_args(args: &[String]) -> Result<Option<Options>, String> {
    let mut inputs = Vec::new();
    let mut output = None;
    let mut dry_run = false;
    let mut check = false;
    let mut morph_triangles = false;
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
            "--morph-triangles" => morph_triangles = true,
            other => {
                if let Some(value) = other.strip_prefix("--output=") {
                    output = Some(PathBuf::from(value));
                } else {
                    return Err(format!("unknown option '{other}'"));
                }
            }
        }
    }
    if !morph_triangles {
        return Err("no pass selected (use --morph-triangles)".to_string());
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
        morph_triangles,
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

fn percent(fraction: Option<f64>) -> String {
    match fraction {
        Some(f) => format!("{:.1}%", f * 100.0),
        None => "no area".to_string(),
    }
}

fn print_part(path: &Path, part: &Part) {
    // A part left as it was is only worth a line when it still folds.
    let still_folds = part
        .folds
        .iter()
        .any(|f| f.after.unwrap_or(0.0) > STILL_FOLDS);
    if part.outcome == PartOutcome::Kept && !still_folds {
        return;
    }
    let name = part
        .mesh_name
        .clone()
        .unwrap_or_else(|| format!("mesh {}", part.mesh));
    let what = match &part.outcome {
        PartOutcome::Retriangulated => "retriangulated".to_string(),
        PartOutcome::Kept => "kept (already least folded)".to_string(),
        PartOutcome::Skipped(why) => format!("skipped: {why}"),
    };
    println!(
        "{}: {name} (primitive {}, {} vertices): {what}",
        path.display(),
        part.primitive,
        part.vertices
    );
    for fold in &part.folds {
        let worst = fold.before.unwrap_or(0.0).max(fold.after.unwrap_or(0.0));
        if worst <= LISTED {
            continue;
        }
        let flag = if fold.after.unwrap_or(0.0) > STILL_FOLDS {
            "  still folds"
        } else {
            ""
        };
        println!(
            "    {:<24} folded {} -> {}{flag}",
            fold.shape,
            percent(fold.before),
            percent(fold.after)
        );
    }
}

fn process_file(path: &Path, options: &Options) -> Result<Outcome, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("read: {e}"))?;
    debug_assert!(
        options.morph_triangles,
        "the only pass, required by parse_args"
    );
    let optimized = optimize_morph_triangles(&bytes).map_err(|e| e.to_string())?;
    for part in &optimized.report.parts {
        print_part(path, part);
    }
    let Some(out_bytes) = optimized.bytes else {
        println!("{}: unchanged", path.display());
        return Ok(Outcome::Unchanged);
    };
    if options.dry_run || options.check {
        println!("{}: would update", path.display());
        return Ok(Outcome::WouldChange);
    }
    match &options.output {
        Some(output) => {
            std::fs::write(output, &out_bytes)
                .map_err(|e| format!("write {}: {e}", output.display()))?;
            println!("{}: updated -> {}", path.display(), output.display());
        }
        None => {
            let backup = backup_path(path);
            std::fs::write(&backup, &bytes)
                .map_err(|e| format!("write backup {}: {e}", backup.display()))?;
            std::fs::write(path, &out_bytes).map_err(|e| format!("write: {e}"))?;
            println!("{}: updated (backup: {})", path.display(), backup.display());
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
    let mut would_change = false;
    for input in &options.inputs {
        match process_file(input, &options) {
            Ok(Outcome::WouldChange) => would_change = true,
            Ok(Outcome::Unchanged | Outcome::Rewritten) => {}
            Err(message) => {
                eprintln!("{}: error: {message}", input.display());
                failed = true;
            }
        }
    }
    if failed {
        ExitCode::from(2)
    } else if options.check && would_change {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}
