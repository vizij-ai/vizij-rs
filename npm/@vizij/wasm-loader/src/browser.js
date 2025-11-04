import { loadBindingsInternal, } from "./shared.js";
async function maybeReadFileBytes(initArg) {
    return initArg;
}
export async function loadBindings(options, initInput) {
    return loadBindingsInternal(options, initInput, maybeReadFileBytes);
}
