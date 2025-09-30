// Stable ESM entry for @vizij/orchestrator-wasm
// Wraps the wasm-pack output in ../../pkg (built with `--target web`).
let _bindings = null;
function pkgWasmJsUrl() {
    return new URL("../../pkg/vizij_orchestrator_wasm.js", import.meta.url);
}
function defaultWasmUrl() {
    return new URL("../../pkg/vizij_orchestrator_wasm_bg.wasm", import.meta.url);
}
async function loadBindings(input) {
    if (!_bindings) {
        const mod = await import(/* @vite-ignore */ pkgWasmJsUrl().toString());
        let initArg = input ?? defaultWasmUrl();
        // Node.js file:// support: read bytes if a file: URL is passed
        try {
            const isUrlObj = typeof initArg === "object" && initArg !== null && "href" in initArg;
            const href = isUrlObj ? initArg.href : (typeof initArg === "string" ? initArg : "");
            const isFileUrl = (isUrlObj && initArg.protocol === "file:") ||
                (typeof href === "string" && href.startsWith("file:"));
            if (isFileUrl) {
                const fsSpec = "node:fs/promises";
                const urlSpec = "node:url";
                const [{ readFile }, { fileURLToPath }] = await Promise.all([
                    import(/* @vite-ignore */ fsSpec),
                    import(/* @vite-ignore */ urlSpec),
                ]);
                const path = fileURLToPath(isUrlObj ? initArg : new URL(href));
                const bytes = await readFile(path);
                initArg = bytes;
            }
        }
        catch {
            // ignore - bundlers handle URLs in the browser
        }
        await mod.default(initArg);
        _bindings = mod;
    }
    return _bindings;
}
/**
 * Initialize the wasm module once.
 */
let _initPromise = null;
export function init(input) {
    if (_initPromise)
        return _initPromise;
    _initPromise = (async () => {
        await loadBindings(input);
    })();
    return _initPromise;
}
function ensureInited() {
    if (!_initPromise) {
        throw new Error("Call init() from @vizij/orchestrator-wasm before creating Orchestrator instances.");
    }
}
/**
 * Ergonomic wrapper around the wasm VizijOrchestrator.
 * Always await init() once before constructing.
 */
export class Orchestrator {
    constructor(opts) {
        ensureInited();
        if (!_bindings) {
            throw new Error("Call init() from @vizij/orchestrator-wasm before creating Orchestrator instances.");
        }
        const Ctor = _bindings.VizijOrchestrator;
        this.inner = new Ctor(opts ?? undefined);
    }
    /**
     * Register a graph controller.
     * Accepts a GraphSpec object or a JSON string or { id?, spec }.
     */
    registerGraph(cfg) {
        const arg = typeof cfg === "string" ? cfg : cfg;
        return this.inner.register_graph(arg);
    }
    /**
     * Register an animation controller.
     * Accepts { id?: string, setup?: any }.
     */
    registerAnimation(cfg) {
        return this.inner.register_animation(cfg);
    }
    /**
     * Prebind resolver used by animation controllers.
     * resolver(path: string) => string|number|null|undefined
     */
    prebind(resolver) {
        const f = (path) => resolver(path);
        this.inner.prebind(f);
    }
    /**
     * Set a blackboard input. value may be a ValueJSON or legacy shape.
     * shape is optional.
     */
    setInput(path, value, shape) {
        const v = value;
        const s = shape ?? undefined;
        this.inner.set_input(path, v, s);
    }
    removeInput(path) {
        return this.inner.remove_input(path);
    }
    /**
     * Step the orchestrator by dt seconds. Returns the OrchestratorFrame (JS object).
     */
    step(dt) {
        return this.inner.step(dt);
    }
    listControllers() {
        return this.inner.list_controllers();
    }
    removeGraph(id) {
        return this.inner.remove_graph(id);
    }
    removeAnimation(id) {
        return this.inner.remove_animation(id);
    }
    /**
     * Normalize a GraphSpec (object or JSON string) using the Rust normalizer.
     */
    async normalizeGraphSpec(spec) {
        await init();
        const mod = await loadBindings();
        const json = typeof spec === "string" ? spec : JSON.stringify(spec);
        const normalized = mod.normalize_graph_spec_json(json);
        return JSON.parse(normalized);
    }
}
export async function createOrchestrator(opts) {
    await init();
    return new Orchestrator(opts);
}
