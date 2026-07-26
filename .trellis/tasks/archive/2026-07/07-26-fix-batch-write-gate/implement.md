# Implementation Plan

1. Extend `ItemTagBatchArgs` with `confirm` and positive-default `max_affected`; add parse cases.
2. Replace the untyped batch JSON builder with private serializable plan/report/outcome types,
   stable validation helpers, and explicit total/selected/sample semantics.
3. Add `BatchTagWriter` plus `WebBatchTagWriter`, enforce the max gate before `ctx.remote()`, and
   implement independent best-effort add/remove attempts across every selected key.
4. Use `AppError::payload()` for nested operation failures so domain and generic codes match the
   top-level JSON contract without verbose chains.
5. Add focused unit/fault-injection tests for validation, preview shape, pre-writer ceiling,
   deterministic attempt order, continuation, outcome counts, and all states.
6. Update canonical `skills/zot/SKILL.md`, bilingual `docs/*/cli/item.md` and safety pages, README,
   and the `zot-cli` quality/error specs; regenerate skill mirrors with `_install-skills`.
7. Run formatting, focused tag/CLI tests, `just skills-check`, workspace clippy, and `just ci`;
   inspect the final diff for unrelated child files and forbidden generalized-ledger scope.

## Validation commands

```powershell
cargo test -p zot-cli tag
cargo test -p zot-cli cli::tests::parses_new_library_and_item_command_surfaces
just _install-skills
just skills-check
cargo clippy --workspace --all-targets -- -D warnings
just ci
```

## Risk and rollback points

- `tag.rs`: a partial report must never hide later calls or stringify typed domain errors.
- `cli/args.rs`: existing scripts become preview-only; docs must show the new second step.
- `--limit` selects targets while `--max-affected` authorizes the selected count; do not compare
  the ceiling to the larger unselected `matched` count.
- Canonical skill edits require generated mirror regeneration; never edit mirrors manually.

## Pre-start checks

- PRD convergence pass complete; no open product questions.
- `task.py validate` passes before `task.py start`.
- Only this task directory is staged for planning; remaining child directories and root audit
  report stay untouched.
