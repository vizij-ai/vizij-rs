//! The Vizij face-bundle toolkit: read and rewrite the `VIZIJ_bundle` a face
//! GLB carries, and validate a face's coverage of the Vizij standard — the
//! `vizij-face` profile.
//!
//! The GLB is a build artifact; the bundle JSON is the reviewable source of
//! truth. `unpack` extracts it as a sidecar, `pack` writes it back, and
//! `add_graph` grafts one graph (e.g. a face's `standard-adaptation`) without
//! touching the rest — all deterministic, so packing is idempotent and diffs
//! stay meaningful.

use anyhow::{anyhow, bail, Context, Result};
use serde_json::{json, Map, Value as Json};
use vizij_arora_host::{mappings, profile};
use vizij_glb_migrate::glb::Glb;

/// A GLB with its parsed JSON chunk, ready for bundle surgery.
pub struct Face {
    glb: Glb,
    pub gltf: Json,
}

impl Face {
    /// Parse a GLB byte buffer.
    pub fn parse(bytes: &[u8]) -> Result<Face> {
        let glb = Glb::parse(bytes).map_err(|e| anyhow!("not a GLB: {e}"))?;
        let gltf: Json = serde_json::from_slice(&glb.json).context("GLB JSON chunk")?;
        Ok(Face { glb, gltf })
    }

    /// The `VIZIJ_bundle` object: on a node's `extensions`, else the document
    /// root's (the same lookup the runtimes use).
    pub fn bundle(&self) -> Option<&Json> {
        self.gltf
            .get("nodes")
            .and_then(Json::as_array)
            .into_iter()
            .flatten()
            .find_map(|node| node.get("extensions").and_then(|e| e.get("VIZIJ_bundle")))
            .or_else(|| {
                self.gltf
                    .get("extensions")
                    .and_then(|e| e.get("VIZIJ_bundle"))
            })
    }

    fn bundle_mut(&mut self) -> Option<&mut Json> {
        let in_node = self
            .gltf
            .get("nodes")
            .and_then(Json::as_array)
            .into_iter()
            .flatten()
            .position(|node| {
                node.get("extensions")
                    .and_then(|e| e.get("VIZIJ_bundle"))
                    .is_some()
            });
        match in_node {
            Some(i) => self.gltf["nodes"][i]["extensions"].get_mut("VIZIJ_bundle"),
            None => self
                .gltf
                .get_mut("extensions")
                .and_then(|e| e.get_mut("VIZIJ_bundle")),
        }
    }

    /// Replace the face's bundle with `bundle`.
    pub fn set_bundle(&mut self, bundle: Json) -> Result<()> {
        let slot = self
            .bundle_mut()
            .ok_or_else(|| anyhow!("the GLB carries no VIZIJ_bundle to replace"))?;
        *slot = bundle;
        Ok(())
    }

    /// Declare a profile on the face: the interface its graphs are authored
    /// against, written to the bundle's top-level `profiles` array in its
    /// portable (unprefixed) form.
    ///
    /// A profile is an interface, not a graph, so it sits beside `graphs`
    /// rather than inside it — the same way an author's imported inputs travel
    /// with the file. Replaces the entry with the same id if present, appends
    /// otherwise, so re-importing updates in place.
    pub fn add_profile(&mut self, profile: &profile::Profile) -> Result<()> {
        let entry = serde_json::to_value(profile).context("serialize the profile")?;
        let bundle = self
            .bundle_mut()
            .ok_or_else(|| anyhow!("the GLB carries no VIZIJ_bundle"))?;
        let map = bundle
            .as_object_mut()
            .ok_or_else(|| anyhow!("the VIZIJ_bundle is not an object"))?;
        let profiles = map
            .entry("profiles")
            .or_insert_with(|| Json::Array(Vec::new()))
            .as_array_mut()
            .ok_or_else(|| anyhow!("the bundle's `profiles` is not an array"))?;
        match profiles.iter().position(|p| p.get("id") == entry.get("id")) {
            Some(i) => profiles[i] = entry,
            None => profiles.push(entry),
        }
        Ok(())
    }

    /// Every profile the face declares, in bundle order. Unlike the runtime
    /// reader, the tool fails on a malformed entry: a face being inspected or
    /// rewritten should not silently lose a declaration.
    pub fn profiles(&self) -> Result<Vec<profile::Profile>> {
        self.bundle()
            .and_then(|b| b.get("profiles"))
            .and_then(Json::as_array)
            .into_iter()
            .flatten()
            .map(|entry| {
                serde_json::from_value(entry.clone()).with_context(|| {
                    let id = entry.get("id").and_then(Json::as_str).unwrap_or("?");
                    format!("the bundle's profile {id:?} is malformed")
                })
            })
            .collect()
    }

    /// Graft one graph entry `{kind, id, spec}` into the bundle: replaces the
    /// entry with the same `id` if present, appends otherwise.
    pub fn add_graph(&mut self, kind: &str, id: &str, spec: Json) -> Result<()> {
        let bundle = self
            .bundle_mut()
            .ok_or_else(|| anyhow!("the GLB carries no VIZIJ_bundle"))?;
        let graphs = bundle
            .get_mut("graphs")
            .and_then(Json::as_array_mut)
            .ok_or_else(|| anyhow!("the bundle carries no graphs array"))?;
        let entry = json!({ "kind": kind, "id": id, "spec": spec });
        match graphs
            .iter()
            .position(|g| g.get("id").and_then(Json::as_str) == Some(id))
        {
            Some(i) => graphs[i] = entry,
            None => graphs.push(entry),
        }
        Ok(())
    }

    /// The prefix of this face's rig input paths — `rig/<faceId>/`, empty when
    /// the bundle declares no face id.
    pub fn rig_prefix(&self) -> String {
        self.bundle()
            .and_then(|b| b.pointer("/metadata/faceId"))
            .and_then(Json::as_str)
            .map(|id| format!("rig/{id}/"))
            .unwrap_or_default()
    }

    /// Embed a shipped standard mapping (e.g. `ros4hri`) into the face: its
    /// control paths get this face's rig prefix, and it grafts under a stable
    /// id (`standard::<mapping>`) so re-embedding replaces rather than
    /// duplicates — the embedded copy stays updatable. Errors on an unknown
    /// mapping id.
    pub fn add_standard_mapping(&mut self, mapping_id: &str) -> Result<()> {
        let (_, spec) = mappings::standard_mapping_source(mapping_id, &self.rig_prefix())
            .ok_or_else(|| anyhow!("unknown standard mapping {mapping_id}"))?;
        self.add_graph(
            mappings::STANDARD_MAPPING_KIND,
            &mappings::embedded_graph_id(mapping_id),
            spec,
        )
    }

    /// Serialize back to GLB bytes (the JSON chunk re-encoded, binary chunks
    /// preserved verbatim).
    pub fn to_bytes(&mut self) -> Result<Vec<u8>> {
        self.glb.json = serde_json::to_vec(&self.gltf).context("encode GLB JSON chunk")?;
        Ok(self.glb.to_bytes())
    }

    /// Every store path the bundle's graphs read (their `input` nodes), with
    /// the rig prefix (`rig/<faceId>/`) stripped — the face's input surface,
    /// in standard vocabulary terms.
    pub fn input_paths(&self) -> Vec<String> {
        let Some(bundle) = self.bundle() else {
            return Vec::new();
        };
        let prefix = bundle
            .pointer("/metadata/faceId")
            .and_then(Json::as_str)
            .map(|id| format!("rig/{id}/"))
            .unwrap_or_default();
        let mut paths: Vec<String> = bundle
            .get("graphs")
            .and_then(Json::as_array)
            .into_iter()
            .flatten()
            .filter_map(|g| g.pointer("/spec/nodes"))
            .filter_map(Json::as_array)
            .flatten()
            .filter(|n| n.get("type").and_then(Json::as_str) == Some("input"))
            .filter_map(|n| n.pointer("/params/path").and_then(Json::as_str))
            .map(|p| p.strip_prefix(&prefix).unwrap_or(p).to_string())
            .collect();
        paths.sort();
        paths.dedup();
        paths
    }
}

/// One tier of the standard-coverage report.
pub struct TierCoverage {
    pub tier: String,
    pub covered: Vec<String>,
    pub missing: Vec<String>,
}

/// A face's coverage of the Vizij standard: which paths of each tier of the
/// `vizij-face` profile its graphs listen on. `level` is the highest tier the
/// face fully covers, in the profile's progression — L0 gaze & lids, L1
/// expressions, L2 visemes, L3 the muscle tier (half is enough there: faces
/// rig the muscles they have).
pub struct Coverage {
    pub face_id: Option<String>,
    pub level: u8,
    pub tiers: Vec<TierCoverage>,
}

/// Compute a face's standard coverage from its input surface, tier by tier of
/// the `vizij-face` profile.
pub fn coverage(face: &Face) -> Coverage {
    let inputs = face.input_paths();
    let has = |path: &str| inputs.iter().any(|p| p == path);

    let standard = profile::vizij_face_profile();
    let tiers: Vec<TierCoverage> = standard
        .tiers()
        .into_iter()
        .map(|tier| {
            let (covered, missing) = standard
                .keys
                .iter()
                .filter(|k| k.tier() == Some(tier))
                .map(|k| k.path.clone())
                .partition(|p| has(p));
            TierCoverage {
                tier: tier.to_string(),
                covered,
                missing,
            }
        })
        .collect();

    // Each level requires every tier before it in full, and its own tier in
    // full — or, for the muscle tier, at least half.
    let mut level = 0;
    for (i, tier) in tiers.iter().enumerate() {
        let full = tier.missing.is_empty();
        let enough = tier.tier == "muscle" && tier.covered.len() >= tier.missing.len();
        if !(full || enough) {
            break;
        }
        level = i as u8;
    }

    Coverage {
        face_id: face
            .bundle()
            .and_then(|b| b.pointer("/metadata/faceId"))
            .and_then(Json::as_str)
            .map(str::to_string),
        level,
        tiers,
    }
}

impl Coverage {
    /// The report as JSON (the machine-readable `validate` output).
    pub fn to_json(&self) -> Json {
        let mut tiers = Map::new();
        for t in &self.tiers {
            tiers.insert(
                t.tier.clone(),
                json!({
                    "covered": t.covered.len(),
                    "of": t.covered.len() + t.missing.len(),
                    "missing": t.missing,
                }),
            );
        }
        json!({
            "faceId": self.face_id,
            "level": format!("L{}", self.level),
            "tiers": tiers,
        })
    }
}

/// A compact inspection summary of a face GLB (the `inspect` output).
pub fn inspect(face: &Face) -> Result<Json> {
    let morphs: Vec<Json> = face
        .gltf
        .get("nodes")
        .and_then(Json::as_array)
        .into_iter()
        .flatten()
        .filter_map(|node| {
            let name = node.get("name").and_then(Json::as_str)?;
            let features = node
                .pointer("/extensions/RobotData/features")?
                .as_object()?;
            let mut names: Vec<&str> = features.keys().map(String::as_str).collect();
            names.sort_unstable();
            Some(json!({ "node": name, "features": names }))
        })
        .collect();
    let graphs: Vec<Json> = face
        .bundle()
        .and_then(|b| b.get("graphs"))
        .and_then(Json::as_array)
        .into_iter()
        .flatten()
        .map(|g| {
            let nodes = g.pointer("/spec/nodes").and_then(Json::as_array);
            json!({
                "kind": g.get("kind"),
                "id": g.get("id"),
                "nodes": nodes.map_or(0, Vec::len),
            })
        })
        .collect();
    // The profiles the face declares — id, version, scope, and how many keys
    // each brings — so `inspect` answers "which interfaces is this face
    // authored against?"
    let profiles: Vec<Json> = face
        .profiles()?
        .iter()
        .map(
            |p| json!({ "id": p.id, "version": p.version, "scope": p.scope, "keys": p.keys.len() }),
        )
        .collect();
    Ok(json!({
        "faceId": face.bundle().and_then(|b| b.pointer("/metadata/faceId")),
        "profiles": profiles,
        "graphs": graphs,
        "inputs": face.input_paths(),
        "animatables": morphs,
    }))
}

/// Pretty-print JSON with a trailing newline — the sidecar format (stable,
/// so unpack → pack round-trips byte-identically).
pub fn to_sidecar(value: &Json) -> Result<String> {
    let mut text = serde_json::to_string_pretty(value)?;
    text.push('\n');
    Ok(text)
}

/// Read a sidecar (or any JSON file) back.
pub fn from_sidecar(text: &str) -> Result<Json> {
    if text.trim().is_empty() {
        bail!("empty sidecar");
    }
    serde_json::from_str(text).context("sidecar JSON")
}

#[cfg(test)]
mod tests {
    use super::*;
    use vizij_arora_host::standard;

    /// A minimal GLB carrying a bundle with one rig graph.
    fn face_bytes(input_paths: &[&str]) -> Vec<u8> {
        let nodes: Vec<Json> = input_paths
            .iter()
            .enumerate()
            .map(|(i, path)| {
                json!({ "id": format!("in{i}"), "type": "input", "params": { "path": path } })
            })
            .collect();
        let gltf = json!({
            "asset": { "version": "2.0" },
            "nodes": [{
                "name": "Scene",
                "extensions": { "VIZIJ_bundle": {
                    "version": 1,
                    "metadata": { "faceId": "test_face" },
                    "graphs": [{ "kind": "rig", "id": "test_rig",
                                 "spec": { "nodes": nodes, "edges": [] } }],
                } },
            }],
        });
        let glb = Glb {
            version: 2,
            json: serde_json::to_vec(&gltf).unwrap(),
            tail: Vec::new(),
        };
        glb.to_bytes()
    }

    #[test]
    fn input_paths_strip_the_rig_prefix() {
        let bytes = face_bytes(&[
            "rig/test_face/standard/vizij/left_eye/pos/x",
            "rig/test_face/custom/thing",
        ]);
        let face = Face::parse(&bytes).unwrap();
        assert_eq!(
            face.input_paths(),
            ["custom/thing", "standard/vizij/left_eye/pos/x"]
        );
    }

    #[test]
    fn add_graph_appends_then_replaces() {
        let bytes = face_bytes(&["rig/test_face/x"]);
        let mut face = Face::parse(&bytes).unwrap();
        face.add_graph(
            "standard-adaptation",
            "adapt",
            json!({ "nodes": [], "edges": [] }),
        )
        .unwrap();
        let count = |f: &Face| f.bundle().unwrap()["graphs"].as_array().unwrap().len();
        assert_eq!(count(&face), 2);
        // Same id replaces, not duplicates.
        face.add_graph(
            "standard-adaptation",
            "adapt",
            json!({ "nodes": [], "edges": [] }),
        )
        .unwrap();
        assert_eq!(count(&face), 2);

        // The grafted bundle survives a GLB round-trip.
        let packed = face.to_bytes().unwrap();
        let reparsed = Face::parse(&packed).unwrap();
        assert_eq!(count(&reparsed), 2);
    }

    /// A declared profile travels with the face like any other authored
    /// input: written to the bundle, replaced in place on re-import, and
    /// intact across a GLB round-trip.
    #[test]
    fn add_profile_declares_the_interface_on_the_face() {
        let bytes = face_bytes(&["rig/test_face/x"]);
        let mut face = Face::parse(&bytes).unwrap();
        assert!(face.profiles().unwrap().is_empty());

        let vizij = profile::profile("vizij-face").unwrap();
        let ros = profile::profile("ros4hri").unwrap();
        face.add_profile(&vizij).unwrap();
        face.add_profile(&ros).unwrap();
        let ids = |f: &Face| {
            f.profiles()
                .unwrap()
                .iter()
                .map(|p| p.id.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(ids(&face), ["vizij-face", "ros4hri"]);

        // Re-importing the same profile updates in place rather than stacking.
        face.add_profile(&vizij).unwrap();
        assert_eq!(ids(&face).len(), 2);

        // It survives the GLB round-trip, keys and all.
        let packed = face.to_bytes().unwrap();
        let reparsed = Face::parse(&packed).unwrap();
        let declared = reparsed.profiles().unwrap();
        assert_eq!(declared.len(), 2);
        assert_eq!(declared[0], vizij);
        assert_eq!(
            inspect(&reparsed).unwrap()["profiles"][1]["scope"],
            "device"
        );
    }

    /// A malformed declaration is an error for the tool, not a silent drop.
    #[test]
    fn a_malformed_profile_fails_the_tool() {
        let bytes = face_bytes(&["rig/test_face/x"]);
        let mut face = Face::parse(&bytes).unwrap();
        let bundle = face.bundle_mut().unwrap();
        bundle["profiles"] = json!([{ "id": "broken", "keys": "not a list" }]);
        let err = face.profiles().unwrap_err().to_string();
        assert!(err.contains("broken"), "{err}");
    }

    #[test]
    fn add_standard_embeds_a_prefixed_updatable_mapping() {
        let bytes = face_bytes(&["rig/test_face/x"]);
        let mut face = Face::parse(&bytes).unwrap();
        face.add_standard_mapping("ros4hri").unwrap();
        assert!(face.add_standard_mapping("nope").is_err());

        let graphs = face.bundle().unwrap()["graphs"].as_array().unwrap();
        let embedded = graphs
            .iter()
            .find(|g| g["id"] == "standard::ros4hri")
            .expect("the mapping embedded");
        assert_eq!(embedded["kind"], "standard-profile");
        // Its outputs carry this face's rig prefix.
        let prefixed = embedded["spec"]["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|n| n["type"] == "output")
            .any(|n| {
                n["params"]["path"]
                    .as_str()
                    .is_some_and(|p| p.starts_with("rig/test_face/standard/vizij/"))
            });
        assert!(prefixed, "mapping outputs are not rig-prefixed");

        // Re-embedding replaces rather than duplicates — the copy is updatable.
        let before = graphs.len();
        face.add_standard_mapping("ros4hri").unwrap();
        assert_eq!(
            face.bundle().unwrap()["graphs"].as_array().unwrap().len(),
            before
        );
    }

    #[test]
    fn coverage_levels_progress_by_tier() {
        // Only two gaze paths: not even L0, and the report names the missing.
        let bytes = face_bytes(&[
            "rig/test_face/standard/vizij/left_eye/pos/x",
            "rig/test_face/standard/vizij/left_eye/pos/y",
        ]);
        let cov = coverage(&Face::parse(&bytes).unwrap());
        assert_eq!(cov.level, 0);
        assert!(cov.tiers[0]
            .missing
            .contains(&standard::RIGHT_EYE_POS_X.to_string()));

        // Full gaze + expressions + visemes: L2.
        let mut paths: Vec<String> = vec![
            standard::LEFT_EYE_POS_X.into(),
            standard::LEFT_EYE_POS_Y.into(),
            standard::RIGHT_EYE_POS_X.into(),
            standard::RIGHT_EYE_POS_Y.into(),
            standard::LEFT_EYE_TOP_EYELID_POS_Y.into(),
            standard::RIGHT_EYE_TOP_EYELID_POS_Y.into(),
        ];
        paths.extend(
            standard::EXPRESSION_NAMES
                .iter()
                .map(|n| standard::expression_path(n)),
        );
        paths.extend(
            standard::VISEME_SHAPES
                .iter()
                .map(|s| standard::viseme_path(s)),
        );
        let prefixed: Vec<String> = paths.iter().map(|p| format!("rig/test_face/{p}")).collect();
        let refs: Vec<&str> = prefixed.iter().map(String::as_str).collect();
        let cov = coverage(&Face::parse(&face_bytes(&refs)).unwrap());
        assert_eq!(cov.level, 2);
        assert_eq!(cov.face_id.as_deref(), Some("test_face"));
    }
}
