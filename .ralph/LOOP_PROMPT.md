You are running ONE iteration of a “fresh-agent” improvement loop on this repository.

FRESH START RULES
- You are a fresh agent. Do NOT rely on prior chat context or hidden state.
- Your only memory is the repository contents and the Ralph artifacts.
- Prefer repo-grounded truth over generic best practices.
- Avoid cosmetic-only edits unless they materially improve accuracy or usability.

DOCSTRING TARGET (RUSTDOC AUTO-GEN)
- Treat all public Rust docstrings as source for auto-generated docs.
- Prefer precise, minimal, correct docs over verbose prose.
- Use consistent conventions: brief summary line, details, examples, panics/errors/safety where relevant.
- Keep examples runnable where possible; note placeholders explicitly if unavoidable.

BROAD-SCOPE ITERATION MODE
- This loop may cover multiple files or modules in one iteration when the goal is large.
- If the goal is broad or nebulous, choose a focused sub-scope (one crate or module group) and proceed without asking for clarification.
- Record your chosen scope and any assumptions in the decision log.
- If blocked, skip that area and continue on another aligned target; do not ask questions unless absolutely blocked.

PROGRESS BUDGET (DEFAULT)
- Aim for a meaningful batch: ~5–15 files or a coherent module slice.
- Prefer depth and correctness over breadth; stop when changes start to get shallow or risky.
- Keep each iteration independently reviewable and commit-worthy.

BACKLOGS (GUIDANCE + TRACKING)

1) PRIMARY TASK BACKLOG (guidance)
- File: .ralph/<task>-backlog.md
- Use this to prioritize work, but you may work beyond a single item when aligned to the goal.
- Keep the Top 10 current.
- Add up to 3 new items per iteration if needed.
- Merge or prune duplicates when you notice them, but no forced quota.

2) OBSERVATION BACKLOG (write-only)
- File: .ralph/observations.md
- Capture out-of-scope bugs, ideas, refactors, tests, or features.
- NEVER execute items from this backlog in this loop.

BACKLOG ITEM FORMAT
- ID: R-###
- Title
- Type: Docs | Code | Process | Tests | Tooling
- Impact: High | Med | Low
- Effort: S | M | L
- Evidence
- Next action
- Status

PROTOCOL
1) Orient: read repo, backlogs, recent commits.
2) Select a scope (crate/module set) and one or more aligned backlog items.
3) Implement docstring improvements across the chosen scope.
4) Verify using the cheapest relevant check.
5) Update:
   - Primary backlog (status updates, Top 10, add ≤3 items if needed)
   - Observation backlog (append only)
   - Decision log (short entry including scope + assumptions)
6) Commit: ralph(<task> iter-XX): <scope> docs

OUTPUT
- Scope chosen
- Files changed
- Verification result
- Backlog updates
- Observations added
- Next recommendation
