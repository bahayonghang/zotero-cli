# Implement: 本地 DB 语义与查询性能

## Ordered Checklist

1. Add core result/build metadata and optional envelope trash policy with serde tests.
2. Flip `SearchOptions` trash default, add CLI `--include-trashed`, update stats predicates and envelope meta.
3. Make collection lookup key-first with deterministic name ambiguity errors and fixture tests.
4. Replace `get_notes` per-note tags with the existing batch loader.
5. Replace Rust candidate-set search with shared SQL count/page queries and page-only batch hydration; update golden regressions.
6. Add minimal duplicate candidate projection, deterministic title blocks, candidate budget/result metadata and dedupe truncation gate.
7. Add graph edge budget/build metadata and truncated human warning.
8. Add `run_local` and migrate the named heavy library/graph/annotation/workspace paths without moving remote work into blocking threads.
9. Add deterministic 10k/50k complexity regressions and update operational limits plus zot-local/zot-cli specs.
10. Complete AC mapping, focused tests, full crate tests, `just ci`, diff review and atomic commit.

## Focused Validation

```powershell
cargo test -p zot-local db::tests
cargo test -p zot-local graph
cargo test -p zot-local --test search_regression
cargo test -p zot-cli cli
cargo test -p zot-cli util
cargo test -p zot-cli library
cargo test -p zot-cli graph
```

## Full Gate

```powershell
just ci
```

## Risk And Rollback Points

- Search SQL must preserve query OR and structured-filter AND semantics; keep legacy fixture results as a golden oracle while replacing the implementation.
- SQL ordering must include a key tie-break before hydration so HashMap iteration cannot affect pages.
- Duplicate group limit and candidate budget are distinct; never reintroduce input truncation through `--limit`.
- A truncated duplicate scan must not reach dedupe writer construction.
- Graph budget limits candidate pairs, not already-admitted signal updates; otherwise relation weights become traversal-order artifacts.
- `spawn_blocking` closures own config/scope/results and must not borrow `AppContext` or run async remote calls.
- Do not touch the other three planning task directories or the parent audit report.
