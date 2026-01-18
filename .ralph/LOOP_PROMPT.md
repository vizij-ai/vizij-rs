
You are running ONE iteration of a “fresh-agent” improvement loop on this repository.

FRESH START RULES
- You are a fresh agent. Do NOT rely on prior chat context or hidden state.
- Your only memory is the repository contents and the Ralph artifacts.
- Prefer repo-grounded truth over generic best practices.
- Avoid cosmetic-only edits unless they materially improve accuracy or usability.

TWO BACKLOGS (MANDATORY)

1) PRIMARY TASK BACKLOG (actionable)
- File: .ralph/<task>-backlog.md
- This is the ONLY place you may select work from.
- Add up to 5 new items per iteration.
- You MUST synthesize/merge 3 existing items per iteration.

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
2) Select ONE item from the primary backlog.
3) Implement minimal in-scope improvement.
4) Verify using cheapest relevant check.
5) Update:
   - Primary backlog (add ≤5, merge 3, update Top 10)
   - Observation backlog (append only)
   - Decision log (short entry)
6) Commit: ralph(<task> iter-XX): summary

OUTPUT
- Chosen item
- Files changed
- Verification result
- Backlog updates
- Observations added
- Next recommendation
