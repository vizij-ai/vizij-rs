export interface PipelineDescriptor {
    description?: string;
    animation: string;
    graph: string;
    initial_inputs?: Array<{
        path: string;
        value: unknown;
    }>;
    steps?: Array<{
        delta: number;
        expect: Record<string, unknown>;
    }>;
    [key: string]: unknown;
}
export interface OrchestrationBundle<TDescriptor extends PipelineDescriptor = PipelineDescriptor, TAnimation = unknown, TGraphSpec = unknown, TGraphStage = unknown> {
    descriptor: TDescriptor;
    animation: TAnimation;
    graphSpec: TGraphSpec;
    graphStage?: TGraphStage | null;
}
export declare function orchestrationNames(): string[];
export declare function orchestrationJson(name: string): string;
export declare function orchestrationDescriptor<T = unknown>(name: string): T;
export declare function orchestrationDescriptorPath(name: string): string;
export declare function loadOrchestrationBundle(name: string): OrchestrationBundle<PipelineDescriptor>;
