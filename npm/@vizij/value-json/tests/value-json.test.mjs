import test from "node:test";
import assert from "node:assert/strict";

import {
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
  const legacyVector = { vector: [12, 13] };

  assert.equal(valueAsNumber(normalizedVector), 4);
  assert.equal(valueAsNumber(3.5), 3.5);
  assert.equal(valueAsNumber(legacyVector), 12);
});

test("valueAsNumericArray normalizes entries", () => {
  const mixed = { type: "vector", data: [1, "oops", 3] };
  assert.deepEqual(valueAsNumericArray(mixed, 10), [1, 10, 3]);
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

  assert.deepEqual(valueAsColorRgba(normalizedColor), [0.5, 0.6, 0.7, 1]);
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
