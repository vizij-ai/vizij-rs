export type AnimationFixture<T = unknown> = T;
export declare function animationNames(): string[];
export declare function animationJson(name: string): string;
export declare function animationFixture<T = unknown>(name: string): AnimationFixture<T>;
export declare function animationPath(name: string): string;
export declare function animationsRoot(): string;
export declare const fixturesDirectory: string;
