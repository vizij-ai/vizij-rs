export type InitInput = string | URL | Uint8Array;
export declare function init(input?: InitInput): Promise<void>;
export type Value = any;
export type ShapeJSON = any;
/**
 * Ergonomic wrapper around the wasm VizijOrchestrator.
 * Always await init() once before constructing.
 */
export declare class Orchestrator {
    private inner;
    constructor(opts?: any);
    /**
     * Register a graph controller.
     * Accepts a GraphSpec object or a JSON string or { id?, spec }.
     */
    registerGraph(cfg: object | string): string;
    /**
     * Register an animation controller.
     * Accepts { id?: string, setup?: any }.
     */
    registerAnimation(cfg: object): string;
    /**
     * Prebind resolver used by animation controllers.
     * resolver(path: string) => string|number|null|undefined
     */
    prebind(resolver: (path: string) => string | number | null | undefined): void;
    /**
     * Set a blackboard input. value may be a ValueJSON or legacy shape.
     * shape is optional.
     */
    setInput(path: string, value: Value, shape?: ShapeJSON): void;
    removeInput(path: string): boolean;
    /**
     * Step the orchestrator by dt seconds. Returns the OrchestratorFrame (JS object).
     */
    step(dt: number): any;
    listControllers(): any;
    removeGraph(id: string): boolean;
    removeAnimation(id: string): boolean;
    /**
     * Normalize a GraphSpec (object or JSON string) using the Rust normalizer.
     */
    normalizeGraphSpec(spec: object | string): Promise<object>;
}
export declare function createOrchestrator(opts?: any): Promise<Orchestrator>;
