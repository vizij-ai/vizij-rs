/**
 * Shared Value JSON helpers and type definitions used across Vizij wasm wrappers.
 */

export type Float = { float: number };
export type Bool = { bool: boolean };
export type Text = { text: string };
export type Vec2 = { vec2: [number, number] };
export type Vec3 = { vec3: [number, number, number] };
export type Vec4 = { vec4: [number, number, number, number] };
export type Quat = { quat: [number, number, number, number] };
export type ColorRgba = { color: [number, number, number, number] };
export type Vector = { vector: number[] };
export type EnumVal = { enum: { tag: string; value: ValueJSON } };
export type RecordVal = { record: { [key: string]: ValueJSON } };
export type ArrayVal = { array: ValueJSON[] };
export type ListVal = { list: ValueJSON[] };
export type TupleVal = { tuple: ValueJSON[] };
export type NormalizedTransform = {
  translation: [number, number, number];
  rotation: [number, number, number, number];
  scale: [number, number, number];
};

export type Transform = {
  transform: {
    translation: [number, number, number];
    rotation: [number, number, number, number];
    scale: [number, number, number];
  };
};

export type NormalizedValue =
  | { type: "float"; data: number }
  | { type: "bool"; data: boolean }
  | { type: "text"; data: string }
  | { type: "vec2"; data: [number, number] }
  | { type: "vec3"; data: [number, number, number] }
  | { type: "vec4"; data: [number, number, number, number] }
  | { type: "quat"; data: [number, number, number, number] }
  | { type: "colorrgba"; data: [number, number, number, number] }
  | { type: "vector"; data: number[] }
  | { type: "enum"; data: [string, NormalizedValue] }
  | { type: "record"; data: { [key: string]: NormalizedValue } }
  | { type: "array"; data: NormalizedValue[] }
  | { type: "list"; data: NormalizedValue[] }
  | { type: "tuple"; data: NormalizedValue[] }
  | { type: "transform"; data: NormalizedTransform };

export type ValueJSON =
  | Float
  | Bool
  | Text
  | Vec2
  | Vec3
  | Vec4
  | Quat
  | ColorRgba
  | Vector
  | EnumVal
  | RecordVal
  | ArrayVal
  | ListVal
  | TupleVal
  | Transform
  | NormalizedValue
  | number
  | string
  | boolean;

export type ValueInput = ValueJSON | number[];

/**
 * Normalize primitive JS values/primitives into the ValueJSON surface.
 * Arrays are encoded as generic vectors to avoid implicit vec2/vec3 coercions.
 */
export function toValueJSON(value: ValueInput): ValueJSON {
  if (typeof value === "number") {
    return { float: value };
  }
  if (typeof value === "boolean") {
    return { bool: value };
  }
  if (typeof value === "string") {
    return { text: value };
  }
  if (Array.isArray(value)) {
    return { vector: value.slice() };
  }
  return value;
}

export function isNormalizedValue(value: ValueJSON): value is NormalizedValue {
  return typeof value === "object" && value !== null && "type" in (value as any) && "data" in (value as any);
}

export function valueAsNumber(value: NormalizedValue | undefined | null): number | undefined {
  if (!value) return undefined;
  switch (value.type) {
    case "float": {
      const num = Number(value.data);
      return Number.isFinite(num) ? num : undefined;
    }
    case "bool":
      return value.data ? 1 : 0;
    case "vec2":
    case "vec3":
    case "vec4":
    case "quat":
    case "colorrgba":
    case "vector": {
      if (!Array.isArray(value.data) || value.data.length === 0) return undefined;
      const num = Number(value.data[0]);
      return Number.isFinite(num) ? num : undefined;
    }
    default:
      return undefined;
  }
}

export function valueAsNumericArray(
  value: NormalizedValue | undefined | null,
  fallback = 0,
): number[] | undefined {
  if (!value) return undefined;
  switch (value.type) {
    case "float": {
      const num = Number(value.data);
      return Number.isFinite(num) ? [num] : [fallback];
    }
    case "bool":
      return [value.data ? 1 : 0];
    case "vec2":
    case "vec3":
    case "vec4":
    case "quat":
    case "colorrgba":
    case "vector":
      return Array.isArray(value.data)
        ? value.data.map((entry) => {
            const num = Number(entry);
            return Number.isFinite(num) ? num : fallback;
          })
        : undefined;
    default:
      return undefined;
  }
}

export function valueAsTransform(
  value: NormalizedValue | undefined | null,
): NormalizedTransform | undefined {
  if (!value || value.type !== "transform") return undefined;
  const { translation, rotation, scale } = value.data;
  if (
    Array.isArray(translation) &&
    Array.isArray(rotation) &&
    Array.isArray(scale) &&
    translation.length === 3 &&
    rotation.length === 4 &&
    scale.length === 3
  ) {
    return {
      translation: translation.map((n) => Number(n) || 0) as [number, number, number],
      rotation: rotation.map((n) => Number(n) || 0) as [number, number, number, number],
      scale: scale.map((n) => Number(n) || 0) as [number, number, number],
    };
  }
  return undefined;
}
