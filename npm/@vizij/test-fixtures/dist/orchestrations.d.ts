export interface StageEntry {
    path: string;
    value: unknown;
    shape?: unknown;
}
export type GraphSeed = string | {
    fixture: string;
    id?: string;
    subs?: Record<string, unknown>;
    mirrorWrites?: boolean;
    stage?: StageEntry[];
};
export type AnimationSeed = string | {
    fixture: string;
    id?: string;
    setup?: Record<string, unknown>;
    player?: Record<string, unknown>;
    instance?: Record<string, unknown>;
};
export interface MergeStrategySeed {
    outputs?: string;
    intermediate?: string;
}
export interface MergedGraphSeed {
    id: string;
    graphs: GraphSeed[];
    strategy?: MergeStrategySeed;
}
export interface PipelineDescriptor {
    description?: string;
    schedule?: string;
    animations?: AnimationSeed[];
    graphs?: GraphSeed[];
    mergedGraphs?: MergedGraphSeed[];
    initial_inputs?: StageEntry[];
    steps?: Array<{
        delta: number;
        expect: Record<string, unknown>;
    }>;
    [key: string]: unknown;
}
export interface OrchestrationGraphBinding<TConfig = Record<string, unknown>> {
    key: string;
    id?: string;
    config: TConfig;
    mirrorWrites: boolean;
    stage: StageEntry[];
}
export type MergeStrategy = "error" | "namespace" | "blend";
export interface OrchestrationMergedGraphStrategy {
    outputs: MergeStrategy;
    intermediate: MergeStrategy;
}
export interface OrchestrationMergedGraphBinding<TConfig = Record<string, unknown>> {
    id: string;
    graphs: Array<OrchestrationGraphBinding<TConfig>>;
    strategy: OrchestrationMergedGraphStrategy;
}
export interface OrchestrationAnimationBinding<TAnimation = Record<string, unknown>, TSetup = Record<string, unknown>> {
    key: string;
    id?: string;
    animation: TAnimation;
    setup: TSetup;
}
export interface OrchestrationBundle<TDescriptor extends PipelineDescriptor = PipelineDescriptor, TAnimation = Record<string, unknown>, TGraphSpec = Record<string, unknown>> {
    descriptor: TDescriptor;
    animations: Array<OrchestrationAnimationBinding<TAnimation>>;
    graphs: Array<OrchestrationGraphBinding<TGraphSpec>>;
    mergedGraphs: Array<OrchestrationMergedGraphBinding<TGraphSpec>>;
    initialInputs: StageEntry[];
}
export declare function orchestrationNames(): string[];
export declare function orchestrationJson(name: string): string;
export declare function orchestrationDescriptor<T = unknown>(name: string): T;
export declare function orchestrationDescriptorPath(name: string): string;
export declare function loadOrchestrationBundle(name: string): OrchestrationBundle<PipelineDescriptor>;
