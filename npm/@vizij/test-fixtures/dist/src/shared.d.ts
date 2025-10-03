export interface NodeGraphManifestEntry {
    spec: string;
    stage?: string;
}
export type OrchestrationManifestEntry = string | {
    path: string;
};
export interface FixturesManifest {
    animations: Record<string, string>;
    "node-graphs": Record<string, NodeGraphManifestEntry>;
    orchestrations: Record<string, OrchestrationManifestEntry>;
}
export declare function fixturesRoot(): string;
export declare function manifest(): FixturesManifest;
export declare function resolveFixturePath(relPath: string): string;
export declare function readFixture(relPath: string): string;
export declare function loadFixture<T>(relPath: string): T;
export declare function animationEntry(name: string): string;
export declare function nodeGraphEntry(name: string): NodeGraphManifestEntry;
export declare function orchestrationEntry(name: string): OrchestrationManifestEntry;
export declare function orchestrationPath(entry: OrchestrationManifestEntry): string;
