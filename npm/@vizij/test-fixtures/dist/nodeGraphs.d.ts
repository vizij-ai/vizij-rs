export interface NodeGraphSpecFixture<TSpec = unknown, TStage = unknown> {
    spec: TSpec;
    stage?: TStage | null;
}
export declare function nodeGraphNames(): string[];
export declare function nodeGraphSpecJson(name: string): string;
export declare function nodeGraphSpec<T = unknown>(name: string): T;
export declare function nodeGraphSpecPath(name: string): string;
export declare function nodeGraphStageJson(name: string): string | null;
export declare function nodeGraphStage<T = unknown>(name: string): T | null;
export declare function nodeGraphStagePath(name: string): string | null;
