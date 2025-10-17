# AroraBlackboard Delivery Plan

## Execution Protocol
Always follow these steps for every task:
1. Work through the tasks sequentially, completing one before starting the next.
2. After finishing a task, run the relevant tests and fix any failures by correcting the code (avoid weakening tests unless a test is demonstrably wrong).
3. Once a task is done, send a chat report summarising outcomes, noting any deviations from the plan or uncovered gaps, and state whether follow-up work was deferred.
4. Conclude each report by asking whether to proceed with the next task, and continue only after approval.

## Epic A – Interface Decomposition & Preparation
- [ ] **A1 – Audit trait boundaries**: Catalogue every `BlackboardInterface` method in `bb/blackboard_ref.rs`, flagging the ones that return `Arc<Mutex<ArcABBNode>>` or otherwise leak thread-safe wrappers.
- [ ] **A2 – Define node-access trait**: Add a new trait (working name `BlackboardNodeAccess`) colocated with `BlackboardInterface` that carries `lookup_node`, `lookup_node_by_id`, and other Arc/Mutex-centric helpers.
- [ ] **A3 – Refactor implementations**: Update `BlackboardRef` and its `Arc<Mutex<_>>` wrapper to implement both `BlackboardInterface` and the new node-access trait; keep compile-time separation clean.
- [ ] **A4 – Plumb through call sites**: Adjust modules/tests that rely on node-level access (e.g., areas instantiating `BlackboardRef`) so they bound against the new trait where necessary.
- [ ] **A5 – Document the split**: Add short rustdoc/comments explaining when to pick the thread-safe API versus the slim interface, covering both `blackboard_ref.rs` and crate-level `README` notes if needed.

## Epic B – Single-Thread AroraBlackboard Implementation
- [ ] **B1 – Design concurrency-lite storage**: Decide on the internal containers for the non-`Arc` variant (e.g., plain structs or `Rc<RefCell>` for graph edges) by reviewing `arc_arora_blackboard.rs`, `abb_node.rs`, and `abb_pathnode.rs` dependencies.
- [ ] **B2 – Abstract shared logic**: Extract reusable operations (ID bookkeeping, path creation, `KeyValue` merging) into helper modules or generic traits so both blackboards consume the same codepaths wherever possible.
- [ ] **B3 – Implement `AroraBlackboard` core**: Introduce the new struct (likely under `src/arora_blackboard.rs`) that mirrors the semantics of `ArcAroraBlackboard` but with single-thread mutability.
- [ ] **B4 – Provide node primitives**: Create non-`Arc`/`Mutex` equivalents for `ArcABBNode` and `ArcABBPathNode` (or make existing types generic over a synchronization wrapper) to support the new blackboard without duplicating business logic.
- [ ] **B5 – Hook into `BlackboardRef`**: Extend `BlackboardType` and construction paths so clients can opt into the single-thread variant while reusing the existing interface/enum wiring.
- [ ] **B6 – Ensure feature parity**: Verify support for JSON serialization (`JsonSerializable`), `KeyValue` ingestion, path creation, and UUID stability matches the thread-safe implementation prior to moving on.

## Epic C – Testing & Verification
- [ ] **C1 – Dual-run existing tests**: Update `crates/blackboard/vizij-blackboard-core/tests/arora_blackboard_tests.rs` so scenarios execute against both `ArcAroraBlackboard` and the new `AroraBlackboard` (table-driven or macro-based to avoid duplication).
- [ ] **C2 – Add targeted coverage**: Introduce unit tests that stress single-thread behaviour (e.g., mutable borrowing patterns, rapid set/overwrite cycles) and confirm it matches previous expectations.
- [ ] **C3 – Stabilize integrations**: Ensure any higher-level consumers (if present in other crates) include the new variant in their smoke tests or fixtures.
- [ ] **C4 – Continuous regression loop**: Run `cargo fmt`, `cargo clippy --all-targets --all-features`, and both scoped and workspace `cargo test` after each milestone, keeping a checklist of failing cases until parity is achieved.
