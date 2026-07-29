//! The `vizij-bundle` CLI. Subcommands:
//!
//! - `inspect <glb>` — face summary as JSON: id, graphs, input surface,
//!   animatable features per node.
//! - `unpack <glb> -o <bundle.json>` — extract the `VIZIJ_bundle` as a
//!   pretty-printed sidecar (the reviewable source of truth).
//! - `pack <glb> --bundle <bundle.json> -o <out.glb>` — write a sidecar back
//!   into a GLB.
//! - `add-graph <glb> --graph <spec.json> --kind <kind> --id <id> -o <out.glb>`
//!   — graft one graph (e.g. a face's `standard-adaptation`) into the bundle,
//!   replacing any entry with the same id.
//! - `add-standard <glb> --standard <profile> -o <out.glb>` — embed a shipped
//!   standard profile (see `profiles`) into the face: the profile's control
//!   paths get the face's rig prefix, and re-adding replaces, so the embedded
//!   copy is updatable.
//! - `validate <glb>` — standard-coverage report (tiers, level, missing
//!   paths) as JSON; exits 1 below `--min-level`.
//! - `profiles` — list the standard profiles Vizij ships, as JSON.
//! - `export-profile <profile> [-o <file.json>]` — regenerate a profile's
//!   canonical asset from its generator (stdout without `-o`).

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{anyhow, bail, Context, Result};
use vizij_arora_host::{profiles, ros4hri};

struct Args {
    command: String,
    /// The positional argument: a GLB path, or a profile id for
    /// `export-profile`.
    target: Option<String>,
    output: Option<PathBuf>,
    bundle: Option<PathBuf>,
    graph: Option<PathBuf>,
    kind: Option<String>,
    id: Option<String>,
    standard: Option<String>,
    min_level: u8,
}

const USAGE: &str = "usage: vizij-bundle <command> …
  inspect        <face.glb>
  unpack         <face.glb> -o <bundle.json>
  pack           <face.glb> --bundle <bundle.json> -o <out.glb>
  add-graph      <face.glb> --graph <spec.json> --kind <kind> --id <id> -o <out.glb>
  add-standard   <face.glb> --standard <profile> -o <out.glb>
  validate       <face.glb> [--min-level <0-3>]
  profiles
  export-profile <profile> [-o <file.json>]";

fn parse_args() -> Result<Args> {
    let mut args = std::env::args().skip(1);
    let command = args.next().ok_or_else(|| anyhow!("{USAGE}"))?;
    let mut target = None;
    let mut output = None;
    let mut bundle = None;
    let mut graph = None;
    let mut kind = None;
    let mut id = None;
    let mut standard = None;
    let mut min_level = 0;
    while let Some(arg) = args.next() {
        let mut value = |name: &str| {
            args.next()
                .ok_or_else(|| anyhow!("{name} expects a value\n{USAGE}"))
        };
        match arg.as_str() {
            "-o" | "--output" => output = Some(PathBuf::from(value("-o")?)),
            "--bundle" => bundle = Some(PathBuf::from(value("--bundle")?)),
            "--graph" => graph = Some(PathBuf::from(value("--graph")?)),
            "--kind" => kind = Some(value("--kind")?),
            "--id" => id = Some(value("--id")?),
            "--standard" => standard = Some(value("--standard")?),
            "--min-level" => min_level = value("--min-level")?.parse().context("--min-level")?,
            "-h" | "--help" => bail!("{USAGE}"),
            _ if target.is_none() => target = Some(arg),
            _ => bail!("unexpected argument {arg}\n{USAGE}"),
        }
    }
    Ok(Args {
        command,
        target,
        output,
        bundle,
        graph,
        kind,
        id,
        standard,
        min_level,
    })
}

/// The commands that work on shipped assets rather than a GLB.
fn run_assets(args: &Args) -> Result<Option<ExitCode>> {
    match args.command.as_str() {
        "profiles" => {
            println!(
                "{}",
                vizij_bundle::to_sidecar(&profiles::standard_profiles_json())?
            );
            Ok(Some(ExitCode::SUCCESS))
        }
        "export-profile" => {
            let id = args
                .target
                .as_deref()
                .ok_or_else(|| anyhow!("export-profile needs a profile id\n{USAGE}"))?;
            profiles::standard_profile(id)
                .ok_or_else(|| anyhow!("unknown profile {id} (see `vizij-bundle profiles`)"))?;
            // One generator today; the registry keys which one to run.
            let spec = match id {
                "ros4hri" => ros4hri::generate(),
                _ => unreachable!("registered profiles have generators"),
            };
            let text = vizij_bundle::to_sidecar(&spec)?;
            match &args.output {
                Some(path) => std::fs::write(path, text)
                    .with_context(|| format!("write {}", path.display()))?,
                None => print!("{text}"),
            }
            Ok(Some(ExitCode::SUCCESS))
        }
        _ => Ok(None),
    }
}

fn run() -> Result<ExitCode> {
    let args = parse_args()?;
    if let Some(code) = run_assets(&args)? {
        return Ok(code);
    }

    let glb = PathBuf::from(
        args.target
            .as_deref()
            .ok_or_else(|| anyhow!("missing <face.glb>\n{USAGE}"))?,
    );
    let bytes = std::fs::read(&glb).with_context(|| format!("read {}", glb.display()))?;
    let mut face = vizij_bundle::Face::parse(&bytes)?;

    match args.command.as_str() {
        "inspect" => {
            println!(
                "{}",
                vizij_bundle::to_sidecar(&vizij_bundle::inspect(&face))?
            );
        }
        "unpack" => {
            let bundle = face
                .bundle()
                .ok_or_else(|| anyhow!("the GLB carries no VIZIJ_bundle"))?;
            let text = vizij_bundle::to_sidecar(bundle)?;
            match &args.output {
                Some(path) => std::fs::write(path, text)
                    .with_context(|| format!("write {}", path.display()))?,
                None => print!("{text}"),
            }
        }
        "pack" => {
            let path = args
                .bundle
                .ok_or_else(|| anyhow!("pack needs --bundle\n{USAGE}"))?;
            let text = std::fs::read_to_string(&path)
                .with_context(|| format!("read {}", path.display()))?;
            face.set_bundle(vizij_bundle::from_sidecar(&text)?)?;
            let out = args
                .output
                .ok_or_else(|| anyhow!("pack needs -o\n{USAGE}"))?;
            std::fs::write(&out, face.to_bytes()?)
                .with_context(|| format!("write {}", out.display()))?;
        }
        "add-graph" => {
            let path = args
                .graph
                .ok_or_else(|| anyhow!("add-graph needs --graph\n{USAGE}"))?;
            let text = std::fs::read_to_string(&path)
                .with_context(|| format!("read {}", path.display()))?;
            let kind = args
                .kind
                .ok_or_else(|| anyhow!("add-graph needs --kind\n{USAGE}"))?;
            let id = args
                .id
                .ok_or_else(|| anyhow!("add-graph needs --id\n{USAGE}"))?;
            face.add_graph(&kind, &id, vizij_bundle::from_sidecar(&text)?)?;
            let out = args
                .output
                .ok_or_else(|| anyhow!("add-graph needs -o\n{USAGE}"))?;
            std::fs::write(&out, face.to_bytes()?)
                .with_context(|| format!("write {}", out.display()))?;
        }
        "add-standard" => {
            let id = args
                .standard
                .ok_or_else(|| anyhow!("add-standard needs --standard\n{USAGE}"))?;
            face.add_standard_profile(&id)?;
            let out = args
                .output
                .ok_or_else(|| anyhow!("add-standard needs -o\n{USAGE}"))?;
            std::fs::write(&out, face.to_bytes()?)
                .with_context(|| format!("write {}", out.display()))?;
        }
        "validate" => {
            let coverage = vizij_bundle::coverage(&face);
            println!("{}", vizij_bundle::to_sidecar(&coverage.to_json())?);
            if coverage.level < args.min_level {
                eprintln!(
                    "coverage L{} is below the required L{}",
                    coverage.level, args.min_level
                );
                return Ok(ExitCode::FAILURE);
            }
        }
        other => bail!("unknown command {other}\n{USAGE}"),
    }
    Ok(ExitCode::SUCCESS)
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(e) => {
            eprintln!("vizij-bundle: {e:#}");
            ExitCode::FAILURE
        }
    }
}
