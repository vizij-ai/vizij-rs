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

const DEFAULT_VEC2: [number, number] = [0, 0];
const DEFAULT_VEC3: [number, number, number] = [0, 0, 0];
const DEFAULT_VEC4: [number, number, number, number] = [0, 0, 0, 0];
const DEFAULT_QUAT: [number, number, number, number] = [0, 0, 0, 1];
const DEFAULT_COLOR: [number, number, number, number] = [0, 0, 0, 0];
const DEFAULT_SCALE: [number, number, number] = [1, 1, 1];

const EMPTY_STRING = "";

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function toFiniteNumber(value: unknown): number | undefined {
  if (typeof value === "number" && Number.isFinite(value)) {
    return value;
  }
  if (typeof value === "string" && value.trim().length > 0) {
    const parsed = Number(value);
    return Number.isFinite(parsed) ? parsed : undefined;
  }
  return undefined;
}

function coerceNumericArray(values: unknown, fallback = 0): number[] | undefined {
  if (!Array.isArray(values)) return undefined;
  return values.map((entry) => toFiniteNumber(entry) ?? fallback);
}

function coerceTuple(
  values: unknown,
  defaults: readonly number[],
): number[] | undefined {
  if (!Array.isArray(values)) return undefined;
  const result = defaults.slice() as number[];
  for (let i = 0; i < defaults.length; i += 1) {
    const num = toFiniteNumber(values[i]);
    if (num !== undefined) {
      result[i] = num;
    }
  }
  return result;
}

function normalizedValueAsNumber(value: NormalizedValue): number | undefined {
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
      const first = value.data[0];
      const num = Number(first);
      return Number.isFinite(num) ? num : undefined;
    }
    case "transform": {
      const first = value.data.translation[0];
      const num = Number(first);
      return Number.isFinite(num) ? num : undefined;
    }
    case "enum":
      return normalizedValueAsNumber(value.data[1]);
    case "array":
    case "list":
    case "tuple":
      return value.data.length > 0 ? normalizedValueAsNumber(value.data[0]) : undefined;
    default:
      return undefined;
  }
}

function normalizedValueAsNumericArray(
  value: NormalizedValue,
  fallback: number,
): number[] | undefined {
  switch (value.type) {
    case "float": {
      const num = Number(value.data);
      return [Number.isFinite(num) ? num : fallback];
    }
    case "bool":
      return [value.data ? 1 : 0];
    case "vec2":
    case "vec3":
    case "vec4":
    case "quat":
    case "colorrgba":
    case "vector":
      return value.data.map((entry) => {
        const num = Number(entry);
        return Number.isFinite(num) ? num : fallback;
      });
    default:
      return undefined;
  }
}

function normalizedValueAsTransform(value: NormalizedValue): NormalizedTransform | undefined {
  if (value.type !== "transform") return undefined;
  const { translation, rotation, scale } = value.data;
  return {
    translation: translation.map((n) => Number(n) || 0) as [number, number, number],
    rotation: rotation.map((n) => Number(n) || 0) as [number, number, number, number],
    scale: scale.map((n) => Number(n) || 0) as [number, number, number],
  };
}

function normalizedValueAsVec3(value: NormalizedValue): [number, number, number] | undefined {
  switch (value.type) {
    case "vec3":
      return [...value.data] as [number, number, number];
    case "vec4":
      return [value.data[0], value.data[1], value.data[2]] as [number, number, number];
    case "quat":
      return [value.data[0], value.data[1], value.data[2]] as [number, number, number];
    case "colorrgba":
      return [value.data[0], value.data[1], value.data[2]] as [number, number, number];
    case "vector":
      return [value.data[0] ?? 0, value.data[1] ?? 0, value.data[2] ?? 0] as [number, number, number];
    case "transform": {
      const pos = value.data.translation;
      return [pos[0] ?? 0, pos[1] ?? 0, pos[2] ?? 0] as [number, number, number];
    }
    case "enum":
      return normalizedValueAsVec3(value.data[1]);
    case "array":
    case "list":
    case "tuple":
      return value.data.length > 0 ? normalizedValueAsVec3(value.data[0]) : undefined;
    case "float":
      return [value.data, value.data, value.data] as [number, number, number];
    case "bool":
      return value.data ? ([1, 1, 1] as [number, number, number]) : ([0, 0, 0] as [number, number, number]);
    default:
      return undefined;
  }
}

function normalizedValueAsVector(value: NormalizedValue): number[] | undefined {
  switch (value.type) {
    case "vector":
      return value.data.slice();
    case "vec2":
      return [value.data[0], value.data[1]];
    case "vec3":
      return [...value.data];
    case "vec4":
      return [...value.data];
    case "quat":
      return [...value.data];
    case "colorrgba":
      return [...value.data];
    case "transform":
      return [
        value.data.translation[0] ?? 0,
        value.data.translation[1] ?? 0,
        value.data.translation[2] ?? 0,
        value.data.rotation[0] ?? 0,
        value.data.rotation[1] ?? 0,
        value.data.rotation[2] ?? 0,
        value.data.rotation[3] ?? 0,
        value.data.scale[0] ?? 1,
        value.data.scale[1] ?? 1,
        value.data.scale[2] ?? 1,
      ];
    case "enum":
      return normalizedValueAsVector(value.data[1]);
    case "array":
    case "list":
    case "tuple":
      return value.data.flatMap((entry) => normalizedValueAsVector(entry) ?? []);
    case "float":
      return [value.data];
    case "bool":
      return [value.data ? 1 : 0];
    default:
      return undefined;
  }
}

function normalizedValueAsBool(value: NormalizedValue): boolean | undefined {
  switch (value.type) {
    case "bool":
      return value.data;
    case "float":
      return value.data !== 0;
    case "text":
      return value.data.length > 0;
    case "vec2":
    case "vec3":
    case "vec4":
    case "quat":
    case "colorrgba":
    case "vector":
      return value.data.some((entry) => Number(entry) !== 0);
    case "transform": {
      const { translation, rotation, scale } = value.data;
      return (
        translation.some((entry) => Number(entry) !== 0) ||
        rotation.some((entry) => Number(entry) !== 0) ||
        scale.some((entry) => Number(entry) !== 0)
      );
    }
    case "enum":
      return normalizedValueAsBool(value.data[1]);
    case "record":
      return Object.values(value.data).some((entry) => normalizedValueAsBool(entry) ?? false);
    case "array":
    case "list":
    case "tuple":
      return value.data.some((entry) => normalizedValueAsBool(entry) ?? false);
    default:
      return undefined;
  }
}

function normalizedValueAsQuat(
  value: NormalizedValue,
): [number, number, number, number] | undefined {
  switch (value.type) {
    case "quat":
      return [...value.data] as [number, number, number, number];
    case "vec4":
      return [value.data[0], value.data[1], value.data[2], value.data[3]] as [number, number, number, number];
    case "vector":
      return [value.data[0] ?? 0, value.data[1] ?? 0, value.data[2] ?? 0, value.data[3] ?? 0] as [
        number,
        number,
        number,
        number,
      ];
    case "transform": {
      const rot = value.data.rotation;
      return [rot[0] ?? 0, rot[1] ?? 0, rot[2] ?? 0, rot[3] ?? 0] as [number, number, number, number];
    }
    case "enum":
      return normalizedValueAsQuat(value.data[1]);
    case "array":
    case "list":
    case "tuple":
      return value.data.length > 0 ? normalizedValueAsQuat(value.data[0]) : undefined;
    default:
      return undefined;
  }
}

function normalizedValueAsColorRgba(
  value: NormalizedValue,
): [number, number, number, number] | undefined {
  switch (value.type) {
    case "colorrgba":
      return [...value.data] as [number, number, number, number];
    case "vec4":
      return [value.data[0], value.data[1], value.data[2], value.data[3]] as [number, number, number, number];
    case "vector":
      return [value.data[0] ?? 0, value.data[1] ?? 0, value.data[2] ?? 0, value.data[3] ?? 0] as [
        number,
        number,
        number,
        number,
      ];
    case "float": {
      const num = Number(value.data) || 0;
      return [num, num, num, 1];
    }
    case "bool":
      return value.data ? [1, 1, 1, 1] : [0, 0, 0, 1];
    case "transform": {
      const scale = value.data.scale;
      return [scale[0] ?? 0, scale[1] ?? 0, scale[2] ?? 0, 1];
    }
    case "enum":
      return normalizedValueAsColorRgba(value.data[1]);
    case "array":
    case "list":
    case "tuple":
      return value.data.length > 0 ? normalizedValueAsColorRgba(value.data[0]) : undefined;
    default:
      return undefined;
  }
}

function normalizedValueAsText(value: NormalizedValue): string | undefined {
  switch (value.type) {
    case "text":
      return value.data;
    case "enum":
      return normalizedValueAsText(value.data[1]);
    case "array":
    case "list":
    case "tuple":
      return value.data.length > 0 ? normalizedValueAsText(value.data[0]) : undefined;
    default:
      return undefined;
  }
}

function legacyValueAsNumber(value: ValueJSON | undefined | null): number | undefined {
  if (value == null) return undefined;
  if (typeof value === "number") {
    return Number.isFinite(value) ? value : undefined;
  }
  if (typeof value === "boolean") {
    return value ? 1 : 0;
  }
  if (Array.isArray(value) && value.length > 0) {
    return legacyValueAsNumber(value[0] as ValueJSON);
  }
  if (!isRecord(value)) return undefined;
  if ("type" in value && "data" in value) {
    // Already normalized, handled elsewhere.
    return undefined;
  }
  if ("float" in value) return legacyValueAsNumber((value as Float).float as ValueJSON);
  if ("bool" in value) return (value as Bool).bool ? 1 : 0;
  if ("vec2" in value) return legacyValueAsNumber((value as Vec2).vec2?.[0] as ValueJSON);
  if ("vec3" in value) return legacyValueAsNumber((value as Vec3).vec3?.[0] as ValueJSON);
  if ("vec4" in value) return legacyValueAsNumber((value as Vec4).vec4?.[0] as ValueJSON);
  if ("quat" in value) return legacyValueAsNumber((value as Quat).quat?.[0] as ValueJSON);
  if ("color" in value) return legacyValueAsNumber((value as ColorRgba).color?.[0] as ValueJSON);
  if ("vector" in value) {
    const vec = (value as Vector).vector;
    return Array.isArray(vec) && vec.length > 0 ? legacyValueAsNumber(vec[0] as ValueJSON) : undefined;
  }
  if ("transform" in value) {
    const transform = (value as Transform).transform;
    if (isRecord(transform)) {
      const translation = (transform.translation as unknown[]) ?? (transform as any).pos;
      if (Array.isArray(translation) && translation.length > 0) {
        return legacyValueAsNumber(translation[0] as ValueJSON);
      }
    }
    return undefined;
  }
  if ("enum" in value) return legacyValueAsNumber((value as EnumVal).enum?.value as ValueJSON);
  if ("array" in value) {
    const arr = (value as ArrayVal).array;
    return Array.isArray(arr) && arr.length > 0 ? legacyValueAsNumber(arr[0] as ValueJSON) : undefined;
  }
  if ("list" in value) {
    const list = (value as ListVal).list;
    return Array.isArray(list) && list.length > 0 ? legacyValueAsNumber(list[0] as ValueJSON) : undefined;
  }
  if ("tuple" in value) {
    const tuple = (value as TupleVal).tuple;
    return Array.isArray(tuple) && tuple.length > 0 ? legacyValueAsNumber(tuple[0] as ValueJSON) : undefined;
  }
  if ("data" in value) return legacyValueAsNumber((value as { data?: unknown }).data as ValueJSON);
  if ("value" in value) return legacyValueAsNumber((value as { value?: unknown }).value as ValueJSON);
  return undefined;
}

function legacyValueAsNumericArray(
  value: ValueJSON | undefined | null,
  fallback: number,
): number[] | undefined {
  if (value == null) return undefined;
  if (typeof value === "number") {
    return [Number.isFinite(value) ? value : fallback];
  }
  if (typeof value === "boolean") {
    return [value ? 1 : 0];
  }
  if (Array.isArray(value)) {
    return value.map((entry) => toFiniteNumber(entry) ?? fallback);
  }
  if (!isRecord(value)) return undefined;
  if ("type" in value && "data" in value) {
    return undefined;
  }
  if ("float" in value) return legacyValueAsNumericArray((value as Float).float as ValueJSON, fallback);
  if ("bool" in value) return legacyValueAsNumericArray((value as Bool).bool as ValueJSON, fallback);
  if ("vec2" in value) return coerceNumericArray((value as Vec2).vec2, fallback);
  if ("vec3" in value) return coerceNumericArray((value as Vec3).vec3, fallback);
  if ("vec4" in value) return coerceNumericArray((value as Vec4).vec4, fallback);
  if ("quat" in value) return coerceNumericArray((value as Quat).quat, fallback);
  if ("color" in value) return coerceNumericArray((value as ColorRgba).color, fallback);
  if ("vector" in value) return coerceNumericArray((value as Vector).vector, fallback);
  return undefined;
}

function legacyValueAsTransform(value: ValueJSON | undefined | null): NormalizedTransform | undefined {
  if (value == null || !isRecord(value)) return undefined;
  if (!("transform" in value)) return undefined;
  const payload = (value as Transform).transform;
  if (!isRecord(payload)) return undefined;
  const translation =
    coerceTuple(payload.translation ?? (payload as any).pos, DEFAULT_VEC3) ?? DEFAULT_VEC3.slice();
  const rotation = coerceTuple(payload.rotation ?? (payload as any).rot, DEFAULT_QUAT) ?? DEFAULT_QUAT.slice();
  const scale = coerceTuple(payload.scale, DEFAULT_SCALE) ?? DEFAULT_SCALE.slice();
  if (!translation || !rotation || !scale) return undefined;
  return {
    translation: translation as [number, number, number],
    rotation: rotation as [number, number, number, number],
    scale: scale as [number, number, number],
  };
}

function legacyValueAsVec3(value: ValueJSON | undefined | null): [number, number, number] | undefined {
  if (value == null) return undefined;
  if (typeof value === "number") {
    return [value, value, value];
  }
  if (typeof value === "boolean") {
    return value ? ([1, 1, 1] as [number, number, number]) : ([0, 0, 0] as [number, number, number]);
  }
  if (Array.isArray(value)) {
    return [value[0] ?? 0, value[1] ?? 0, value[2] ?? 0] as [number, number, number];
  }
  if (!isRecord(value)) return undefined;
  if ("type" in value && "data" in value) {
    return undefined;
  }
  if ("vec3" in value) return (value as Vec3).vec3 as [number, number, number];
  if ("vec4" in value) {
    const vec4 = (value as Vec4).vec4;
    return [vec4[0], vec4[1], vec4[2]] as [number, number, number];
  }
  if ("quat" in value) {
    const quat = (value as Quat).quat;
    return [quat[0], quat[1], quat[2]] as [number, number, number];
  }
  if ("color" in value) {
    const color = (value as ColorRgba).color;
    return [color[0], color[1], color[2]] as [number, number, number];
  }
  if ("vector" in value) {
    const vec = (value as Vector).vector;
    return [vec[0] ?? 0, vec[1] ?? 0, vec[2] ?? 0] as [number, number, number];
  }
  if ("transform" in value) {
    const payload = (value as Transform).transform;
    if (isRecord(payload)) {
      const pos = (payload.translation as number[]) ?? (payload as any).pos ?? DEFAULT_VEC3;
      return [pos[0] ?? 0, pos[1] ?? 0, pos[2] ?? 0] as [number, number, number];
    }
  }
  if ("enum" in value) return legacyValueAsVec3((value as EnumVal).enum?.value as ValueJSON);
  if ("array" in value) {
    const array = (value as ArrayVal).array;
    return Array.isArray(array) && array.length > 0 ? legacyValueAsVec3(array[0] as ValueJSON) : undefined;
  }
  if ("list" in value) {
    const list = (value as ListVal).list;
    return Array.isArray(list) && list.length > 0 ? legacyValueAsVec3(list[0] as ValueJSON) : undefined;
  }
  if ("tuple" in value) {
    const tuple = (value as TupleVal).tuple;
    return Array.isArray(tuple) && tuple.length > 0 ? legacyValueAsVec3(tuple[0] as ValueJSON) : undefined;
  }
  if ("float" in value) return legacyValueAsVec3((value as Float).float as ValueJSON);
  if ("bool" in value) return legacyValueAsVec3((value as Bool).bool as ValueJSON);
  return undefined;
}

function legacyValueAsVector(value: ValueJSON | undefined | null): number[] | undefined {
  if (value == null) return undefined;
  if (typeof value === "number") {
    return Number.isFinite(value) ? [value] : undefined;
  }
  if (typeof value === "boolean") {
    return [value ? 1 : 0];
  }
  if (Array.isArray(value)) {
    return value.map((entry) => toFiniteNumber(entry) ?? 0);
  }
  if (!isRecord(value)) return undefined;
  if ("type" in value && "data" in value) {
    return undefined;
  }
  if ("vector" in value) {
    const vec = (value as Vector).vector;
    return Array.isArray(vec) ? vec.map((entry) => toFiniteNumber(entry) ?? 0) : undefined;
  }
  if ("vec2" in value) return [...((value as Vec2).vec2 ?? DEFAULT_VEC2)] as number[];
  if ("vec3" in value) return [...((value as Vec3).vec3 ?? DEFAULT_VEC3)] as number[];
  if ("vec4" in value) return [...((value as Vec4).vec4 ?? DEFAULT_VEC4)] as number[];
  if ("quat" in value) return [...((value as Quat).quat ?? DEFAULT_QUAT)] as number[];
  if ("color" in value) return [...((value as ColorRgba).color ?? DEFAULT_COLOR)] as number[];
  if ("transform" in value) {
    const payload = (value as Transform).transform;
    if (isRecord(payload)) {
      const pos = (payload.translation as number[]) ?? (payload as any).pos ?? DEFAULT_VEC3;
      const rot = (payload.rotation as number[]) ?? (payload as any).rot ?? DEFAULT_QUAT;
      const scale = (payload.scale as number[]) ?? DEFAULT_SCALE;
      return [
        pos[0] ?? 0,
        pos[1] ?? 0,
        pos[2] ?? 0,
        rot[0] ?? 0,
        rot[1] ?? 0,
        rot[2] ?? 0,
        rot[3] ?? 0,
        scale[0] ?? 1,
        scale[1] ?? 1,
        scale[2] ?? 1,
      ];
    }
  }
  if ("enum" in value) return legacyValueAsVector((value as EnumVal).enum?.value as ValueJSON);
  if ("array" in value) {
    const arr = (value as ArrayVal).array;
    if (Array.isArray(arr)) {
      return arr.flatMap((entry) => legacyValueAsVector(entry as ValueJSON) ?? []);
    }
  }
  if ("list" in value) {
    const list = (value as ListVal).list;
    if (Array.isArray(list)) {
      return list.flatMap((entry) => legacyValueAsVector(entry as ValueJSON) ?? []);
    }
  }
  if ("tuple" in value) {
    const tuple = (value as TupleVal).tuple;
    if (Array.isArray(tuple)) {
      return tuple.flatMap((entry) => legacyValueAsVector(entry as ValueJSON) ?? []);
    }
  }
  if ("float" in value) {
    const num = legacyValueAsNumber((value as Float).float as ValueJSON);
    return num !== undefined ? [num] : undefined;
  }
  if ("bool" in value) {
    return (value as Bool).bool ? [1] : [0];
  }
  return undefined;
}

function legacyValueAsBool(value: ValueJSON | undefined | null): boolean | undefined {
  if (value == null) return undefined;
  if (typeof value === "boolean") return value;
  if (typeof value === "number") return value !== 0;
  if (typeof value === "string") return value.length > 0;
  if (Array.isArray(value)) {
    return value.some((entry) => (toFiniteNumber(entry) ?? 0) !== 0);
  }
  if (!isRecord(value)) return undefined;
  if ("type" in value && "data" in value) {
    return undefined;
  }
  if ("bool" in value) return (value as Bool).bool;
  if ("float" in value) return ((value as Float).float ?? 0) !== 0;
  if ("text" in value) return ((value as Text).text ?? "").length > 0;
  if ("vector" in value) {
    const vec = (value as Vector).vector;
    return Array.isArray(vec) ? vec.some((entry) => (entry as number) !== 0) : undefined;
  }
  if ("vec2" in value) return (value as Vec2).vec2?.some((entry) => entry !== 0);
  if ("vec3" in value) return (value as Vec3).vec3?.some((entry) => entry !== 0);
  if ("vec4" in value) return (value as Vec4).vec4?.some((entry) => entry !== 0);
  if ("quat" in value) return (value as Quat).quat?.some((entry) => entry !== 0);
  if ("color" in value) return (value as ColorRgba).color?.some((entry) => entry !== 0);
  if ("transform" in value) {
    const payload = (value as Transform).transform;
    if (isRecord(payload)) {
      const translation = (payload.translation as number[]) ?? (payload as any).pos ?? [];
      const rotation = (payload.rotation as number[]) ?? (payload as any).rot ?? [];
      const scale = (payload.scale as number[]) ?? [];
      return (
        translation.some((entry) => entry !== 0) ||
        rotation.some((entry) => entry !== 0) ||
        scale.some((entry) => entry !== 0)
      );
    }
    return undefined;
  }
  if ("enum" in value) return legacyValueAsBool((value as EnumVal).enum?.value as ValueJSON) ?? false;
  if ("record" in value) {
    const recordVal = (value as RecordVal).record;
    return Object.values(recordVal ?? {}).some((entry) => legacyValueAsBool(entry as ValueJSON) ?? false);
  }
  if ("array" in value) {
    const arr = (value as ArrayVal).array;
    return Array.isArray(arr) ? arr.some((entry) => legacyValueAsBool(entry as ValueJSON) ?? false) : undefined;
  }
  if ("list" in value) {
    const list = (value as ListVal).list;
    return Array.isArray(list) ? list.some((entry) => legacyValueAsBool(entry as ValueJSON) ?? false) : undefined;
  }
  if ("tuple" in value) {
    const tuple = (value as TupleVal).tuple;
    return Array.isArray(tuple) ? tuple.some((entry) => legacyValueAsBool(entry as ValueJSON) ?? false) : undefined;
  }
  if ("data" in value) return legacyValueAsBool((value as { data?: unknown }).data as ValueJSON);
  if ("value" in value) return legacyValueAsBool((value as { value?: unknown }).value as ValueJSON);
  return undefined;
}

function legacyValueAsQuat(
  value: ValueJSON | undefined | null,
): [number, number, number, number] | undefined {
  if (value == null) return undefined;
  if (Array.isArray(value)) {
    return [value[0] ?? 0, value[1] ?? 0, value[2] ?? 0, value[3] ?? 0] as [number, number, number, number];
  }
  if (!isRecord(value)) return undefined;
  if ("type" in value && "data" in value) {
    return undefined;
  }
  if ("quat" in value) return (value as Quat).quat;
  if ("vec4" in value) return (value as Vec4).vec4;
  if ("vector" in value) {
    const vec = (value as Vector).vector;
    return [vec[0] ?? 0, vec[1] ?? 0, vec[2] ?? 0, vec[3] ?? 0] as [number, number, number, number];
  }
  if ("transform" in value) {
    const payload = (value as Transform).transform;
    if (isRecord(payload)) {
      const rot = (payload.rotation as number[]) ?? (payload as any).rot ?? DEFAULT_QUAT;
      return [rot[0] ?? 0, rot[1] ?? 0, rot[2] ?? 0, rot[3] ?? 0] as [number, number, number, number];
    }
  }
  if ("enum" in value) return legacyValueAsQuat((value as EnumVal).enum?.value as ValueJSON);
  if ("array" in value) {
    const arr = (value as ArrayVal).array;
    return Array.isArray(arr) && arr.length > 0 ? legacyValueAsQuat(arr[0] as ValueJSON) : undefined;
  }
  if ("list" in value) {
    const list = (value as ListVal).list;
    return Array.isArray(list) && list.length > 0 ? legacyValueAsQuat(list[0] as ValueJSON) : undefined;
  }
  if ("tuple" in value) {
    const tuple = (value as TupleVal).tuple;
    return Array.isArray(tuple) && tuple.length > 0 ? legacyValueAsQuat(tuple[0] as ValueJSON) : undefined;
  }
  return undefined;
}

function legacyValueAsColorRgba(
  value: ValueJSON | undefined | null,
): [number, number, number, number] | undefined {
  if (value == null) return undefined;
  if (Array.isArray(value)) {
    return [value[0] ?? 0, value[1] ?? 0, value[2] ?? 0, value[3] ?? 0] as [number, number, number, number];
  }
  if (typeof value === "number") {
    return [value, value, value, 1];
  }
  if (typeof value === "boolean") {
    return value ? [1, 1, 1, 1] : [0, 0, 0, 1];
  }
  if (!isRecord(value)) return undefined;
  if ("type" in value && "data" in value) {
    return undefined;
  }
  if ("bool" in value) return (value as Bool).bool ? [1, 1, 1, 1] : [0, 0, 0, 1];
  if ("float" in value) {
    const num = (value as Float).float ?? 0;
    return [num, num, num, 1];
  }
  if ("color" in value) return (value as ColorRgba).color;
  if ("vec4" in value) return (value as Vec4).vec4;
  if ("vector" in value) {
    const vec = (value as Vector).vector;
    return [vec[0] ?? 0, vec[1] ?? 0, vec[2] ?? 0, vec[3] ?? 0] as [number, number, number, number];
  }
  if ("transform" in value) {
    const payload = (value as Transform).transform;
    if (isRecord(payload)) {
      const scale = (payload.scale as number[]) ?? DEFAULT_SCALE;
      return [scale[0] ?? 0, scale[1] ?? 0, scale[2] ?? 0, 1];
    }
  }
  if ("enum" in value) return legacyValueAsColorRgba((value as EnumVal).enum?.value as ValueJSON);
  if ("array" in value) {
    const arr = (value as ArrayVal).array;
    return Array.isArray(arr) && arr.length > 0 ? legacyValueAsColorRgba(arr[0] as ValueJSON) : undefined;
  }
  if ("list" in value) {
    const list = (value as ListVal).list;
    return Array.isArray(list) && list.length > 0 ? legacyValueAsColorRgba(list[0] as ValueJSON) : undefined;
  }
  if ("tuple" in value) {
    const tuple = (value as TupleVal).tuple;
    return Array.isArray(tuple) && tuple.length > 0 ? legacyValueAsColorRgba(tuple[0] as ValueJSON) : undefined;
  }
  return undefined;
}

function legacyValueAsText(value: ValueJSON | undefined | null): string | undefined {
  if (value == null) return undefined;
  if (typeof value === "string") return value;
  if (!isRecord(value)) return undefined;
  if ("type" in value && "data" in value) {
    return undefined;
  }
  if ("text" in value) return (value as Text).text ?? EMPTY_STRING;
  if ("enum" in value) return legacyValueAsText((value as EnumVal).enum?.value as ValueJSON);
  if ("array" in value) {
    const arr = (value as ArrayVal).array;
    return Array.isArray(arr) && arr.length > 0 ? legacyValueAsText(arr[0] as ValueJSON) : undefined;
  }
  if ("list" in value) {
    const list = (value as ListVal).list;
    return Array.isArray(list) && list.length > 0 ? legacyValueAsText(list[0] as ValueJSON) : undefined;
  }
  if ("tuple" in value) {
    const tuple = (value as TupleVal).tuple;
    return Array.isArray(tuple) && tuple.length > 0 ? legacyValueAsText(tuple[0] as ValueJSON) : undefined;
  }
  if ("data" in value) return legacyValueAsText((value as { data?: unknown }).data as ValueJSON);
  if ("value" in value) return legacyValueAsText((value as { value?: unknown }).value as ValueJSON);
  return undefined;
}

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

export function valueAsNumber(value: ValueJSON | undefined | null): number | undefined {
  if (value == null) return undefined;
  if (isNormalizedValue(value)) {
    return normalizedValueAsNumber(value);
  }
  return legacyValueAsNumber(value);
}

export function valueAsNumericArray(
  value: ValueJSON | undefined | null,
  fallback = 0,
): number[] | undefined {
  if (value == null) return undefined;
  if (isNormalizedValue(value)) {
    return normalizedValueAsNumericArray(value, fallback);
  }
  return legacyValueAsNumericArray(value, fallback);
}

export function valueAsTransform(
  value: ValueJSON | undefined | null,
): NormalizedTransform | undefined {
  if (value == null) return undefined;
  if (isNormalizedValue(value)) {
    return normalizedValueAsTransform(value);
  }
  return legacyValueAsTransform(value);
}

export function valueAsVec3(
  value: ValueJSON | undefined | null,
): [number, number, number] | undefined {
  if (value == null) return undefined;
  if (isNormalizedValue(value)) {
    return normalizedValueAsVec3(value);
  }
  return legacyValueAsVec3(value);
}

export function valueAsVector(value: ValueJSON | undefined | null): number[] | undefined {
  if (value == null) return undefined;
  if (isNormalizedValue(value)) {
    return normalizedValueAsVector(value);
  }
  return legacyValueAsVector(value);
}

export function valueAsBool(value: ValueJSON | undefined | null): boolean | undefined {
  if (value == null) return undefined;
  if (isNormalizedValue(value)) {
    return normalizedValueAsBool(value);
  }
  return legacyValueAsBool(value);
}

export function valueAsQuat(
  value: ValueJSON | undefined | null,
): [number, number, number, number] | undefined {
  if (value == null) return undefined;
  if (isNormalizedValue(value)) {
    return normalizedValueAsQuat(value);
  }
  return legacyValueAsQuat(value);
}

export function valueAsColorRgba(
  value: ValueJSON | undefined | null,
): [number, number, number, number] | undefined {
  if (value == null) return undefined;
  if (isNormalizedValue(value)) {
    return normalizedValueAsColorRgba(value);
  }
  return legacyValueAsColorRgba(value);
}

export function valueAsText(value: ValueJSON | undefined | null): string | undefined {
  if (value == null) return undefined;
  if (isNormalizedValue(value)) {
    return normalizedValueAsText(value);
  }
  return legacyValueAsText(value);
}
