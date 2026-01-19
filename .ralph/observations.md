
# Ralph Observations (write-only)

Use this file to capture out-of-scope findings such as:
- Code bugs
- Refactor ideas
- Missing tests
- Performance issues
- Future features

Do NOT implement these in the current loop.
- Observed many public APIs with sparse doc comments; consider batching a doc audit pass per crate to identify missing rustdoc examples.
- Node-graph public APIs (types, schema registry, plan cache) still lack runnable examples; consider adding minimal doctests in a future pass once inputs and fixtures are standardized.
