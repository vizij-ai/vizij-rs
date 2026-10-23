import test from "node:test";
import assert from "node:assert/strict";

import {
  fromAroraValueJSON,
  isNormalizedValue,
  toValueJSON,
  valueAsBool,
  valueAsColorRgba,
  valueAsNumber,
  valueAsNumericArray,
  valueAsQuat,
  valueAsText,
  valueAsTransform,
  valueAsVec3,
  valueAsVector,
} from "../dist/index.js";

const sampleTransform = {
  translation: [1, 2, 3],
  rotation: [0, 0, 0, 1],
  scale: [1, 1, 1],
};

test("toValueJSON wraps primitives", () => {
  assert.deepEqual(toValueJSON(2), { float: 2 });
  assert.deepEqual(toValueJSON(true), { bool: true });
  assert.deepEqual(toValueJSON("ok"), { text: "ok" });
});

test("toValueJSON wraps arrays", () => {
  assert.deepEqual(toValueJSON([1, 2]), { vector: [1, 2] });
});

test("toValueJSON passes ValueJSON through", () => {
  const value = { float: 1 };
  assert.equal(toValueJSON(value), value);
});

test("value helpers extract numeric data", () => {
  const normalizedVector = { type: "vec3", data: [4, 5, 6] };
  const uppercaseFloat = { type: "Float", data: 2 };
  const legacyVector = { vector: [12, 13] };

  assert.equal(valueAsNumber(normalizedVector), 4);
  assert.equal(valueAsNumber(3.5), 3.5);
  assert.equal(valueAsNumber(uppercaseFloat), 2);
  assert.equal(valueAsNumber(legacyVector), 12);
});

test("valueAsNumericArray normalizes entries", () => {
  const mixed = { type: "vector", data: [1, "oops", 3] };
  const uppercase = { type: "Vector", data: [4, 5] };
  assert.deepEqual(valueAsNumericArray(mixed, 10), [1, 10, 3]);
  assert.deepEqual(valueAsNumericArray(uppercase, 0), [4, 5]);
  assert.deepEqual(valueAsNumericArray({ bool: true }, 5), [1]);
});

test("valueAsTransform handles both shapes", () => {
  const normalizedTransform = { type: "transform", data: sampleTransform };
  const legacyTransform = {
    transform: {
      translation: [7, 8, 9],
      rotation: [0.1, 0.2, 0.3, 0.4],
      scale: [2, 3, 4],
    },
  };

  assert.deepEqual(valueAsTransform(normalizedTransform), sampleTransform);
  assert.deepEqual(valueAsTransform(legacyTransform), {
    translation: [7, 8, 9],
    rotation: [0.1, 0.2, 0.3, 0.4],
    scale: [2, 3, 4],
  });
});

test("valueAsVec3 extracts vectors", () => {
  const normalizedVector = { type: "vec3", data: [4, 5, 6] };
  const legacyTransform = {
    transform: {
      translation: [7, 8, 9],
      rotation: [0, 0, 0, 1],
      scale: [1, 1, 1],
    },
  };

  assert.deepEqual(valueAsVec3(normalizedVector), [4, 5, 6]);
  assert.deepEqual(valueAsVec3(legacyTransform), [7, 8, 9]);
  assert.deepEqual(valueAsVec3({ float: 2 }), [2, 2, 2]);
});

test("valueAsVector flattens transforms", () => {
  const normalizedTransform = { type: "transform", data: sampleTransform };
  assert.deepEqual(valueAsVector(normalizedTransform), [
    ...sampleTransform.translation,
    ...sampleTransform.rotation,
    ...sampleTransform.scale,
  ]);
  assert.deepEqual(valueAsVector({ vector: [5, 6, 7] }), [5, 6, 7]);
});

test("valueAsBool inspects nested shapes", () => {
  const nested = {
    record: {
      off: { bool: false },
      on: { float: 0.5 },
    },
  };

  assert.equal(valueAsBool({ bool: false }), false);
  assert.equal(valueAsBool(nested), true);
});

test("valueAsQuat extracts quaternion values", () => {
  const normalizedTransform = { type: "transform", data: sampleTransform };
  const legacyTransform = {
    transform: {
      translation: [0, 0, 0],
      rotation: [0.1, 0.2, 0.3, 0.4],
      scale: [1, 1, 1],
    },
  };

  assert.deepEqual(valueAsQuat(normalizedTransform), [0, 0, 0, 1]);
  assert.deepEqual(valueAsQuat(legacyTransform), [0.1, 0.2, 0.3, 0.4]);
  assert.deepEqual(valueAsQuat({ vector: [9, 8, 7, 6] }), [9, 8, 7, 6]);
});

test("valueAsColorRgba extracts color intent", () => {
  const normalizedColor = { type: "colorrgba", data: [0.5, 0.6, 0.7, 1] };
  const uppercaseColor = { type: "COLORRGBA", data: [0.1, 0.2, 0.3, 0.4] };

  assert.deepEqual(valueAsColorRgba(normalizedColor), [0.5, 0.6, 0.7, 1]);
  assert.deepEqual(valueAsColorRgba(uppercaseColor), [0.1, 0.2, 0.3, 0.4]);
  assert.deepEqual(valueAsColorRgba({ bool: true }), [1, 1, 1, 1]);
  assert.deepEqual(valueAsColorRgba({ color: [0.2, 0.3, 0.4, 0.5] }), [0.2, 0.3, 0.4, 0.5]);
});

test("valueAsText extracts strings", () => {
  const normalizedText = { type: "text", data: "hello" };

  assert.equal(valueAsText(normalizedText), "hello");
  assert.equal(valueAsText("legacy"), "legacy");
  assert.equal(valueAsText({ text: "wrapped" }), "wrapped");
});

test("isNormalizedValue type guard", () => {
  const normalized = { type: "float", data: 1 };
  assert.equal(isNormalizedValue(normalized), true);
  assert.equal(isNormalizedValue({ float: 1 }), false);
});

// ---------------------------------------------------------------------------
// Arora serde forms — the JSON the Rust side emits (pinned by vizij-api-core's
// `json_wire_form_matches_the_js_decoder` test).
// ---------------------------------------------------------------------------

const VEC3_TYPE = "76697a69-6a00-0000-0000-000000000003";
const QUAT_TYPE = "76697a69-6a00-0000-0000-000000000010";
const TRANSFORM_TYPE = "76697a69-6a00-0000-0000-000000000030";

function aroraVec3(x, y, z) {
  return {
    struct: {
      id: VEC3_TYPE,
      fields: [
        { id: "76697a69-6a00-0000-0000-000000030001", value: { f32: x } },
        { id: "76697a69-6a00-0000-0000-000000030002", value: { f32: y } },
        { id: "76697a69-6a00-0000-0000-000000030003", value: { f32: z } },
      ],
    },
  };
}

function aroraQuat(x, y, z, w) {
  return {
    struct: {
      id: QUAT_TYPE,
      fields: [
        { id: "76697a69-6a00-0000-0000-000000100001", value: { f32: x } },
        { id: "76697a69-6a00-0000-0000-000000100002", value: { f32: y } },
        { id: "76697a69-6a00-0000-0000-000000100003", value: { f32: z } },
        { id: "76697a69-6a00-0000-0000-000000100004", value: { f32: w } },
      ],
    },
  };
}

test("arora scalars decode through the accessors", () => {
  assert.equal(valueAsNumber({ f32: 1.5 }), 1.5);
  assert.equal(valueAsNumber({ f64: 2.5 }), 2.5);
  assert.equal(valueAsNumber({ u32: 42 }), 42);
  assert.equal(valueAsNumber({ i64: -7 }), -7);
  assert.equal(valueAsBool({ bool: true }), true);
  assert.equal(valueAsText({ str: "hello" }), "hello");
});

test("arora numeric arrays decode as vectors", () => {
  assert.deepEqual(valueAsVector({ f32s: [1, 2, 3] }), [1, 2, 3]);
  assert.deepEqual(valueAsVector({ f64s: [4, 5] }), [4, 5]);
  assert.deepEqual(valueAsNumericArray({ u8s: [7, 8] }), [7, 8]);
});

test("arora vizij structures decode by type id", () => {
  assert.deepEqual(valueAsVec3(aroraVec3(1, 2, 3)), [1, 2, 3]);
  assert.deepEqual(valueAsQuat(aroraQuat(0.1, 0.2, 0.3, 0.4)), [0.1, 0.2, 0.3, 0.4]);
  // Unknown structure ids have no vizij reading.
  assert.equal(
    valueAsNumber({ struct: { id: "00000000-0000-0000-0000-000000009999", fields: [] } }),
    undefined,
  );
});

test("arora transform decodes nested structures", () => {
  const transform = {
    struct: {
      id: TRANSFORM_TYPE,
      fields: [
        { id: "76697a69-6a00-0000-0000-000000300001", value: aroraVec3(1, 2, 3) },
        { id: "76697a69-6a00-0000-0000-000000300002", value: aroraQuat(0, 0, 0, 1) },
        { id: "76697a69-6a00-0000-0000-000000300003", value: aroraVec3(1, 1, 1) },
      ],
    },
  };
  assert.deepEqual(valueAsTransform(transform), {
    translation: [1, 2, 3],
    rotation: [0, 0, 0, 1],
    scale: [1, 1, 1],
  });
  assert.deepEqual(valueAsVec3(transform), [1, 2, 3]);
});

test("arora enums decode with the variant id as tag", () => {
  const value = {
    enum: {
      id: "76697a69-6a00-0000-0000-000000000040",
      variant_id: "0e37eee1-878d-4c07-9b7d-6d5116f8f4c4",
      value: { f32: 3 },
    },
  };
  assert.equal(valueAsNumber(value), 3);
  const decoded = fromAroraValueJSON(value);
  assert.equal(decoded.type, "enum");
  assert.equal(decoded.data[0], "0e37eee1-878d-4c07-9b7d-6d5116f8f4c4");
});

test("arora keyvalue decodes as a record", () => {
  const value = {
    keyvalue: {
      id: "76697a69-6a00-0000-0000-000000000050",
      fields: {
        speed: { id: "00000000-0000-0000-0000-000000000001", name: "speed", value: { f32: 2 } },
        label: { id: "00000000-0000-0000-0000-000000000002", name: "label", value: { str: "go" } },
      },
    },
  };
  const decoded = fromAroraValueJSON(value);
  assert.equal(decoded.type, "record");
  assert.deepEqual(decoded.data.speed, { type: "float", data: 2 });
  assert.deepEqual(decoded.data.label, { type: "text", data: "go" });
});

test("arora heterogeneous arrays and options decode", () => {
  assert.deepEqual(valueAsVector({ values: [{ f32: 1 }, { f32: 2 }] }), [1, 2]);
  assert.equal(valueAsNumber({ option: { f32: 5 } }), 5);
  assert.equal(valueAsNumber({ option: null }), undefined);
  assert.equal(valueAsNumber("unit"), undefined);
});

test("legacy forms still take their own path", () => {
  assert.equal(valueAsNumber({ float: 9 }), 9);
  assert.deepEqual(fromAroraValueJSON({ float: 9 }), undefined);
  // A legacy enum has a string tag and no variant_id — not arora-shaped.
  assert.deepEqual(fromAroraValueJSON({ enum: { tag: "on", value: { float: 1 } } }), undefined);
});
