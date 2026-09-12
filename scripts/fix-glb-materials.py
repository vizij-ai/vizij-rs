"""Correct the PBR fields in a robot GLB, and say what changed.

`metallicFactor` is 0.5 and `roughnessFactor` is 0.5 on every shape RobotData
calls `phong`. Neither is derived from the authored material: Phong has no
metalness at all, and the shininess RobotData still carries would give a
roughness of 0.25, not 0.5. At metalness 0.5 half the base colour stops being
diffuse and becomes specular reflectance, which in a scene with no environment
map simply goes missing.

Both are rewritten here — metalness to 0, roughness from shininess as
`sqrt(2 / (shininess + 2))`. That conversion is a fit, not an authority: if the
real metalness and roughness are supplied, use those and delete this.

RobotData is corrected alongside the glTF material, because the two describe
the same surface and a consumer may read either.

**Emissive is deliberately left alone.** A part authored with a black base
colour *and* a near-white emissive is not a contradiction — it is how a
self-lit element is written, so that it glows evenly and scene lighting cannot
touch it. On this robot those parts are the lamp bands at the top of the pole
and around the base, and the ring around the chest button. Zeroing them turns
the lights off.
"""

import json
import math
import struct
import sys

JSON_CHUNK = 0x4E4F534A
BIN_CHUNK = 0x004E4942


def read_glb(path):
    with open(path, "rb") as f:
        magic, version, _total = struct.unpack("<III", f.read(12))
        assert magic == 0x46546C67, f"not a GLB: {magic:#x}"
        chunks = []
        while True:
            header = f.read(8)
            if len(header) < 8:
                break
            length, kind = struct.unpack("<II", header)
            chunks.append((kind, f.read(length)))
    return version, chunks


def write_glb(path, version, chunks):
    body = b""
    for kind, data in chunks:
        pad = (-len(data)) % 4
        data = data + (b" " if kind == JSON_CHUNK else b"\0") * pad
        body += struct.pack("<II", len(data), kind) + data
    with open(path, "wb") as f:
        f.write(struct.pack("<III", 0x46546C67, version, 12 + len(body)) + body)


def resting(value):
    """A feature's value, whether plain or wrapped in an animatable."""
    if isinstance(value, dict):
        if "default" in value:
            return resting(value["default"])
        if "f32" in value:
            return value["f32"]
    return value


def set_feature(features, key, number):
    """Writes a feature back in whichever shape it already has."""
    slot = features.get(key)
    if not isinstance(slot, dict) or "value" not in slot:
        return False
    value = slot["value"]
    if isinstance(value, dict) and isinstance(value.get("default"), dict):
        if "f32" in value["default"]:
            value["default"]["f32"] = number
            return True
        return False
    slot["value"] = number
    return True


def main(source, destination):
    version, chunks = read_glb(source)
    kinds = [kind for kind, _ in chunks]
    gltf = json.loads(chunks[kinds.index(JSON_CHUNK)][1])

    nodes = gltf.get("nodes", [])
    meshes = gltf.get("meshes", [])
    materials = gltf.get("materials", [])

    # Which RobotData node drives which material, so shininess can be found.
    node_of_material = {}
    for node in nodes:
        data = node.get("extensions", {}).get("RobotData")
        mesh_index = node.get("mesh")
        if data is None or mesh_index is None or mesh_index >= len(meshes):
            continue
        for primitive in meshes[mesh_index].get("primitives", []):
            index = primitive.get("material")
            if index is not None:
                node_of_material.setdefault(index, []).append(data)

    lamps = relit = smoothed = 0
    for index, material in enumerate(materials):
        pbr = material.setdefault("pbrMetallicRoughness", {})
        owners = node_of_material.get(index, [])
        phong = any(d.get("material") == "phong" for d in owners)
        base = pbr.get("baseColorFactor", [1, 1, 1, 1])[:3]
        emissive = material.get("emissiveFactor", [0, 0, 0])

        # Reported, not touched: these are the robot's lamps.
        if any(e > 0.01 for e in emissive) and all(c <= 0.01 for c in base):
            name = next((d.get("name") for d in owners if d.get("name")), f"#{index}")
            tags = next((d.get("tags") for d in owners if d.get("tags")), [])
            print(
                f"  lamp: {name!r} {tags} glows "
                f"{tuple(round(e, 3) for e in emissive)} — left as authored"
            )
            lamps += 1

        if not phong:
            continue

        if pbr.get("metallicFactor") not in (0, 0.0):
            pbr["metallicFactor"] = 0.0
            relit += 1

        shininess = next(
            (
                resting(d["features"]["shininess"].get("value"))
                for d in owners
                if isinstance(d.get("features", {}).get("shininess"), dict)
            ),
            None,
        )
        if isinstance(shininess, (int, float)) and shininess >= 0:
            pbr["roughnessFactor"] = round(math.sqrt(2.0 / (shininess + 2.0)), 4)
            smoothed += 1

    print(
        f"\n{relit} de-metalled, {smoothed} given a roughness from shininess, "
        f"{lamps} lamps left alone"
    )

    chunks[kinds.index(JSON_CHUNK)] = (JSON_CHUNK, json.dumps(gltf).encode("utf-8"))
    write_glb(destination, version, chunks)
    print(f"wrote {destination}")


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
