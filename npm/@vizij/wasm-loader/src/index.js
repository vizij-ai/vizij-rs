import { loadBindingsInternal, } from "./shared.js";
async function maybeReadFileBytes(initArg) {
    const isUrlObject = typeof initArg === "object" && initArg !== null && "href" in initArg;
    const href = isUrlObject
        ? initArg.href
        : typeof initArg === "string"
            ? initArg
            : "";
    const isFileUrl = (isUrlObject && initArg.protocol === "file:") ||
        (typeof href === "string" && href.startsWith("file:"));
    if (!isFileUrl) {
        return initArg;
    }
    try {
        if (typeof window !== "undefined") {
            return initArg;
        }
        const maybeProcess = globalThis?.process;
        if (!maybeProcess?.versions?.node) {
            return initArg;
        }
        const importDynamic = new Function("specifier", "return import(specifier);");
        const [{ readFile }, { fileURLToPath }] = await Promise.all([
            importDynamic("fs/promises"),
            importDynamic("url"),
        ]);
        const path = isUrlObject
            ? fileURLToPath(initArg)
            : fileURLToPath(new URL(href));
        const bytes = await readFile(path);
        return bytes;
    }
    catch {
        return initArg;
    }
}
export async function loadBindings(options, initInput) {
    return loadBindingsInternal(options, initInput, maybeReadFileBytes);
}
