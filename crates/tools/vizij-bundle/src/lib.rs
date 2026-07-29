//! The Vizij face-bundle toolkit: read and rewrite the `VIZIJ_bundle` a face
//! GLB carries, and validate a face's coverage of the Vizij standard.
//!
//! The GLB is a build artifact; the bundle JSON is the reviewable source of
//! truth. `unpack` extracts it as a sidecar, `pack` writes it back, and
//! `add_graph` grafts one graph (e.g. a face's `standard-adaptation`) without
//! touching the rest — all deterministic, so packing is idempotent and diffs
//! stay meaningful.

use anyhow::{anyhow, bail, Context, Result};
use serde_json::{json, Map, Value as Json};
use vizij_arora_host::{profiles, standard};
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

    /// Embed a shipped standard profile (e.g. `ros4hri`) into the face: its
    /// control paths get this face's rig prefix, and it grafts under a stable
    /// id (`standard::<profile>`) so re-embedding replaces rather than
    /// duplicates — the embedded copy stays updatable. Errors on an unknown
    /// profile id.
    pub fn add_standard_profile(&mut self, profile_id: &str) -> Result<()> {
        let (_, spec) = profiles::standard_profile_source(profile_id, &self.rig_prefix())
            .ok_or_else(|| anyhow!("unknown standard profile {profile_id}"))?;
        self.add_graph(
            profiles::STANDARD_PROFILE_KIND,
            &profiles::embedded_graph_id(profile_id),
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
    pub tier: &'static str,
    pub covered: Vec<String>,
    pub missing: Vec<String>,
}

/// A face's coverage of the Vizij standard: which control paths of each tier
/// its graphs listen on. `level` is the highest tier the face fully covers,
/// in the standard's progression — L0 gaze & lids, L1 expressions, L2
/// visemes, L3 the muscle tier (half is enough there: faces rig the muscles
/// they have).
pub struct Coverage {
    pub face_id: Option<String>,
    pub level: u8,
    pub tiers: Vec<TierCoverage>,
}

/// Compute a face's standard coverage from its input surface.
pub fn coverage(face: &Face) -> Coverage {
    let inputs = face.input_paths();
    let has = |path: &str| inputs.iter().any(|p| p == path);
    let split = |paths: Vec<String>| -> (Vec<String>, Vec<String>) {
        paths.into_iter().partition(|p| has(p))
    };

    let gaze_paths = vec![
        standard::LEFT_EYE_POS_X.to_string(),
        standard::LEFT_EYE_POS_Y.to_string(),
        standard::RIGHT_EYE_POS_X.to_string(),
        standard::RIGHT_EYE_POS_Y.to_string(),
        standard::LEFT_EYE_TOP_EYELID_POS_Y.to_string(),
        standard::RIGHT_EYE_TOP_EYELID_POS_Y.to_string(),
    ];
    let expression_paths: Vec<String> = standard::EXPRESSION_NAMES
        .iter()
        .map(|n| standard::expression_path(n))
        .collect();
    let viseme_paths: Vec<String> = standard::VISEME_SHAPES
        .iter()
        .map(|s| standard::viseme_path(s))
        .collect();
    let muscle_paths: Vec<String> = standard::FACE_CONTROLS
        .iter()
        .map(|c| standard::face_path(c.name))
        .collect();

    let (g_cov, g_miss) = split(gaze_paths);
    let (e_cov, e_miss) = split(expression_paths);
    let (v_cov, v_miss) = split(viseme_paths);
    let (m_cov, m_miss) = split(muscle_paths);

    let mut level = 0;
    let l0 = g_miss.is_empty();
    if l0 && e_miss.is_empty() {
        level = 1;
        if v_miss.is_empty() {
            level = 2;
            if m_cov.len() >= m_miss.len() {
                level = 3;
            }
        }
    }

    Coverage {
        face_id: face
            .bundle()
            .and_then(|b| b.pointer("/metadata/faceId"))
            .and_then(Json::as_str)
            .map(str::to_string),
        level,
        tiers: vec![
            TierCoverage {
                tier: "gaze",
                covered: g_cov,
                missing: g_miss,
            },
            TierCoverage {
                tier: "expression",
                covered: e_cov,
                missing: e_miss,
            },
            TierCoverage {
                tier: "viseme",
                covered: v_cov,
                missing: v_miss,
            },
            TierCoverage {
                tier: "muscle",
                covered: m_cov,
                missing: m_miss,
            },
        ],
    }
}

impl Coverage {
    /// The report as JSON (the machine-readable `validate` output).
    pub fn to_json(&self) -> Json {
        let mut tiers = Map::new();
        for t in &self.tiers {
            tiers.insert(
                t.tier.to_string(),
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
pub fn inspect(face: &Face) -> Json {
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
    json!({
        "faceId": face.bundle().and_then(|b| b.pointer("/metadata/faceId")),
        "graphs": graphs,
        "inputs": face.input_paths(),
        "animatables": morphs,
    })
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

    #[test]
    fn add_standard_embeds_a_prefixed_updatable_profile() {
        let bytes = face_bytes(&["rig/test_face/x"]);
        let mut face = Face::parse(&bytes).unwrap();
        face.add_standard_profile("ros4hri").unwrap();
        assert!(face.add_standard_profile("nope").is_err());

        let graphs = face.bundle().unwrap()["graphs"].as_array().unwrap();
        let embedded = graphs
            .iter()
            .find(|g| g["id"] == "standard::ros4hri")
            .expect("the profile embedded");
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
        assert!(prefixed, "profile outputs are not rig-prefixed");

        // Re-embedding replaces rather than duplicates — the copy is updatable.
        let before = graphs.len();
        face.add_standard_profile("ros4hri").unwrap();
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
