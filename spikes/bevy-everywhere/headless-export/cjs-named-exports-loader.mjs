// lodash ships a UMD build whose named exports Node's CJS lexer cannot see;
// serve `lodash` as a synthetic ES module over the CJS default import.
import { createRequire } from "node:module";
import { pathToFileURL } from "node:url";
const require = createRequire("/Users/victor.paleologue/Code/Semio/vizij-web/packages/@vizij/render/src/index.tsx");
const LODASH = pathToFileURL(require.resolve("lodash")).href;
const NAMES = ["mapValues", "omit", "cloneDeep", "isEqual", "merge", "get", "set", "pick", "debounce", "throttle", "uniq", "groupBy", "sortBy", "keyBy", "flatten", "range", "isEmpty", "capitalize", "camelCase", "kebabCase", "snakeCase", "startCase", "isNil", "isString", "isNumber", "isObject", "isArray", "noop", "identity", "uniqueId", "memoize"];
export async function resolve(specifier, context, next) {
  if (specifier === "lodash") return { url: "lodash-esm-shim:lodash", format: "module", shortCircuit: true };
  return next(specifier, context);
}
export async function load(url, context, next) {
  if (url === "lodash-esm-shim:lodash") {
    const source = `import _ from ${JSON.stringify(LODASH)};\nexport default _;\n` + NAMES.map((n) => `export const ${n} = _.${n};`).join("\n");
    return { format: "module", source, shortCircuit: true };
  }
  return next(url, context);
}
