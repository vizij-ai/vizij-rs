//! What the integration tests share: the GLBs they build in memory.

use uuid::Uuid;

/// The GLB of a one-plate face as Semio Studio exports it: an unnamed node
/// carrying a unit square, a `phong` material with three's exporter's
/// metallic factor of 0.5, and RobotData features per axis — `translation.x`
/// and `translation.y` and `rotation.y` (yaw) each on an animatable of its
/// own, `color.r/g/b` on the components of the compound animatable `color`.
/// `bundle`, when given, is the face's `VIZIJ_bundle`.
pub fn studio_face_glb(
    x: Uuid,
    y: Uuid,
    yaw: Uuid,
    color: Uuid,
    bundle: Option<serde_json::Value>,
) -> Vec<u8> {
    let axis = |id: String| serde_json::json!({ "animated": true, "value": { "id": id, "type": "number", "default": 0 } });
    let fixed = |value: f32| serde_json::json!({ "animated": false, "value": value });
    let mut json = serde_json::json!({
        "asset": { "version": "2.0" },
        "extensionsUsed": ["RobotData"],
        "scene": 0,
        "scenes": [{ "nodes": [0] }],
        "nodes": [{
            "mesh": 0,
            "extensions": { "RobotData": {
                "id": Uuid::new_v4().to_string(),
                "name": "Plate",
                "type": "shape",
                "material": "phong",
                "features": {
                    "translation.x": axis(x.to_string()),
                    "translation.y": axis(y.to_string()),
                    "translation.z": fixed(0.0),
                    "rotation.r": fixed(0.0),
                    "rotation.p": fixed(0.0),
                    "rotation.y": axis(yaw.to_string()),
                    "color.r": axis(format!("{color}.r")),
                    "color.g": axis(format!("{color}.g")),
                    "color.b": axis(format!("{color}.b")),
                    "shininess": fixed(30.0),
                }
            } }
        }],
        "meshes": [{ "primitives": [{ "attributes": { "POSITION": 0 }, "indices": 1, "material": 0 }] }],
        "materials": [{ "pbrMetallicRoughness": { "metallicFactor": 0.5, "roughnessFactor": 0.5 } }],
        "accessors": [
            { "bufferView": 0, "componentType": 5126, "count": 4, "type": "VEC3",
              "min": [-0.5, -0.5, 0.0], "max": [0.5, 0.5, 0.0] },
            { "bufferView": 1, "componentType": 5123, "count": 6, "type": "SCALAR" },
        ],
        "bufferViews": [
            { "buffer": 0, "byteOffset": 0, "byteLength": 48 },
            { "buffer": 0, "byteOffset": 48, "byteLength": 12 },
        ],
        "buffers": [{ "byteLength": 60 }],
    });
    if let Some(bundle) = bundle {
        json["extensions"] = serde_json::json!({ "VIZIJ_bundle": bundle });
    }
    let mut bin = Vec::new();
    for p in [
        [-0.5f32, -0.5, 0.0],
        [0.5, -0.5, 0.0],
        [0.5, 0.5, 0.0],
        [-0.5, 0.5, 0.0],
    ] {
        for c in p {
            bin.extend_from_slice(&c.to_le_bytes());
        }
    }
    for i in [0u16, 1, 2, 0, 2, 3] {
        bin.extend_from_slice(&i.to_le_bytes());
    }
    glb(&json, &bin)
}

/// A GLB container of `json` and its binary chunk `bin`.
pub fn glb(json: &serde_json::Value, bin: &[u8]) -> Vec<u8> {
    let mut json = serde_json::to_vec(json).unwrap();
    while !json.len().is_multiple_of(4) {
        json.push(b' ');
    }
    let total = 12 + 8 + json.len() + 8 + bin.len();
    let mut glb = Vec::with_capacity(total);
    glb.extend_from_slice(b"glTF");
    glb.extend_from_slice(&2u32.to_le_bytes());
    glb.extend_from_slice(&(total as u32).to_le_bytes());
    glb.extend_from_slice(&(json.len() as u32).to_le_bytes());
    glb.extend_from_slice(b"JSON");
    glb.extend_from_slice(&json);
    glb.extend_from_slice(&(bin.len() as u32).to_le_bytes());
    glb.extend_from_slice(b"BIN\0");
    glb.extend_from_slice(bin);
    glb
}
