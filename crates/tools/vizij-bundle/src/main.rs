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
//! - `add-standard <glb> --standard <mapping> -o <out.glb>` — embed a shipped
//!   standard mapping (see `mappings`) into the face: the mapping's control
//!   paths get the face's rig prefix, and re-adding replaces, so the embedded
//!   copy is updatable.
//! - `add-profile <glb> --profile <id> -o <out.glb>` — declare a shipped
//!   profile on the face: the interface its graphs are authored against.
//! - `validate <glb>` — standard-coverage report (tiers, level, missing
//!   paths) as JSON; exits 1 below `--min-level`.
//! - `profiles` / `export-profile <id> [-o <file.json>]` — list the profiles
//!   Vizij ships / regenerate one's canonical asset from its generator.
//! - `mappings` / `export-mapping <id> [-o <file.json>]` — the same for the
//!   standard mappings.
//! - `surface <graph.json> --side <input|output> [--scope <device|face>]
//!   --id <id>` — lift the profile a mapping graph consumes or produces.
//! - `export-skill <skill> [-o <file.json>]` — regenerate a skill fragment.
//!
//! Every export writes to stdout without `-o`.

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{anyhow, bail, Context, Result};
use vizij_arora_host::profile::{Scope, Side};
use vizij_arora_host::{mappings, profile, skills};

struct Args {
    command: String,
    /// The positional argument: a GLB path, a graph spec path, or a registry
    /// id for the export commands.
    target: Option<String>,
    output: Option<PathBuf>,
    bundle: Option<PathBuf>,
    graph: Option<PathBuf>,
    kind: Option<String>,
    id: Option<String>,
    standard: Option<String>,
    profile: Option<String>,
    side: Side,
    scope: Scope,
    min_level: u8,
}

const USAGE: &str = "usage: vizij-bundle <command> …
  inspect        <face.glb>
  unpack         <face.glb> -o <bundle.json>
  pack           <face.glb> --bundle <bundle.json> -o <out.glb>
  add-graph      <face.glb> --graph <spec.json> --kind <kind> --id <id> -o <out.glb>
  add-standard   <face.glb> --standard <mapping> -o <out.glb>
  add-profile    <face.glb> --profile <id> -o <out.glb>
  validate       <face.glb> [--min-level <0-3>]
  profiles
  export-profile <profile> [-o <file.json>]
  mappings
  export-mapping <mapping> [-o <file.json>]
  surface        <graph.json> --side <input|output> [--scope <device|face>] --id <id> [-o <file.json>]
  export-skill   <skill>   [-o <file.json>]";

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
    let mut profile = None;
    let mut side = Side::Input;
    let mut scope = Scope::Device;
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
            "--profile" => profile = Some(value("--profile")?),
            "--side" => {
                side = value("--side")?
                    .parse()
                    .map_err(|e| anyhow!("--side {e}"))?
            }
            "--scope" => {
                scope = match value("--scope")?.as_str() {
                    "device" => Scope::Device,
                    "face" => Scope::Face,
                    other => bail!("--scope expects device or face, got {other}"),
                }
            }
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
        profile,
        side,
        scope,
        min_level,
    })
}

/// Write a JSON payload to `output`, or to stdout when it is absent — the
/// shape every export command shares.
fn emit(payload: &serde_json::Value, output: &Option<PathBuf>) -> Result<()> {
    let text = vizij_bundle::to_sidecar(payload)?;
    match output {
        Some(path) => {
            std::fs::write(path, text).with_context(|| format!("write {}", path.display()))?
        }
        None => print!("{text}"),
    }
    Ok(())
}

/// The commands that work on shipped assets rather than a GLB.
fn run_assets(args: &Args) -> Result<Option<ExitCode>> {
    let target = |what: &str| {
        args.target
            .as_deref()
            .ok_or_else(|| anyhow!("{} needs {what}\n{USAGE}", args.command))
    };
    match args.command.as_str() {
        "profiles" => emit(&profile::profiles_json(), &None)?,
        "mappings" => emit(&mappings::standard_mappings_json(), &None)?,
        // Regenerate from the registry's generator: this is how a committed
        // asset is refreshed when the code behind it moves, and the drift
        // test then holds the two equal.
        "export-profile" => {
            let id = target("a profile id")?;
            let entry = profile::shipped(id)
                .ok_or_else(|| anyhow!("unknown profile {id} (see `vizij-bundle profiles`)"))?;
            emit(&serde_json::to_value((entry.generate)())?, &args.output)?;
        }
        "export-mapping" => {
            let id = target("a mapping id")?;
            let entry = mappings::standard_mapping(id)
                .ok_or_else(|| anyhow!("unknown mapping {id} (see `vizij-bundle mappings`)"))?;
            emit(&(entry.generate)(), &args.output)?;
        }
        "export-skill" => {
            let id = target("a skill id")?;
            let spec = match id {
                "look_at" => skills::generate_look_at(),
                "play_viseme" => skills::generate_play_viseme(),
                "say" => skills::generate_say(),
                _ => bail!("unknown skill {id}"),
            };
            emit(&spec, &args.output)?;
        }
        // A mapping graph already names both profiles it touches: its `input`
        // nodes are the interface it consumes, its `output` nodes the one it
        // produces. Lifting either side out is how an existing mapping is
        // reconciled against a declared profile.
        "surface" => {
            let path = target("a graph spec path")?;
            let text = std::fs::read_to_string(path).with_context(|| format!("read {path}"))?;
            let spec: serde_json::Value =
                serde_json::from_str(&text).with_context(|| format!("parse {path}"))?;
            let id = args.id.as_deref().unwrap_or("surface");
            let lifted = profile::surface(&spec, args.side, id, args.scope);
            emit(&serde_json::to_value(&lifted)?, &args.output)?;
        }
        _ => return Ok(None),
    }
    Ok(Some(ExitCode::SUCCESS))
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
        "inspect" => emit(&vizij_bundle::inspect(&face)?, &None)?,
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
            face.add_standard_mapping(&id)?;
            let out = args
                .output
                .ok_or_else(|| anyhow!("add-standard needs -o\n{USAGE}"))?;
            std::fs::write(&out, face.to_bytes()?)
                .with_context(|| format!("write {}", out.display()))?;
        }
        // Declare a profile on the face: the interface its graphs are
        // authored against travels with the asset, so a reader knows which
        // interface to hold the face to.
        "add-profile" => {
            let id = args
                .profile
                .ok_or_else(|| anyhow!("add-profile needs --profile <id>\n{USAGE}"))?;
            let declared = profile::profile(&id)
                .ok_or_else(|| anyhow!("unknown profile {id} (see `vizij-bundle profiles`)"))?;
            face.add_profile(&declared)?;
            let out = args
                .output
                .ok_or_else(|| anyhow!("add-profile needs -o\n{USAGE}"))?;
            std::fs::write(&out, face.to_bytes()?)
                .with_context(|| format!("write {}", out.display()))?;
        }
        "validate" => {
            let coverage = vizij_bundle::coverage(&face);
            emit(&coverage.to_json(), &None)?;
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
