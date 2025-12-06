export async function loadBindingsInternal(options, initInput, maybeReadFileBytes) {
    if (options.cache.current) {
        return options.cache.current;
    }
    const module = await options.importModule();
    let initArg = typeof initInput === "undefined"
        ? options.defaultWasmUrl()
        : initInput;
    initArg = await maybeReadFileBytes(initArg);
    await options.init(module, initArg);
    const bindings = options.getBindings
        ? options.getBindings(module)
        : module;
    if (typeof options.expectedAbi === "number" &&
        typeof options.getAbiVersion === "function") {
        const abi = options.getAbiVersion(bindings);
        if (abi !== options.expectedAbi) {
            throw new Error(`@vizij/wasm-loader ABI mismatch: expected ${options.expectedAbi}, got ${abi}. ` +
                "Rebuild the wasm package and ensure bindings are up to date.");
        }
    }
    options.cache.current = bindings;
    return bindings;
}
