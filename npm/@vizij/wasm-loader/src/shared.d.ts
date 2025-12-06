export type InitInput = string | URL | ArrayBufferView | ArrayBuffer | WebAssembly.Module | Response;
export interface LoadBindingsOptions<TBindings> {
    cache: {
        current: TBindings | null;
    };
    importModule: () => Promise<any>;
    defaultWasmUrl: () => URL | string;
    init: (module: any, initArg: unknown) => Promise<void>;
    getBindings?: (module: any) => TBindings;
    expectedAbi?: number;
    getAbiVersion?: (bindings: TBindings) => number;
}
export declare function loadBindingsInternal<TBindings>(options: LoadBindingsOptions<TBindings>, initInput: InitInput | undefined, maybeReadFileBytes: (initArg: unknown) => Promise<unknown>): Promise<TBindings>;
