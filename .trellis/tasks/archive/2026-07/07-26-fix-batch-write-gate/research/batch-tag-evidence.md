# Batch tag evidence

## Live behavior

- `commands/item/tag.rs`: validates only filter/operation presence, searches locally, constructs
  `ctx.remote()` unconditionally, then uses `?` inside sequential add/remove loops. A failure
  loses the record of prior remote writes and stops later keys.
- `cli/args.rs`: `ItemTagBatchArgs` has query/tag/add/remove/limit but no confirmation or ceiling.
- `zot-core::SearchResult` exposes both `total` and limited `items`; no database API change is
  required to report full matches and selected targets separately.
- `library_dedupe::apply_dedupe_plan` already demonstrates the local convention of a narrow
  async writer seam, per-unit failure capture, continuation, and structured apply report.

## Operator contract drift

- `skills/zot/SKILL.md` classifies tag batch as layer-C high-risk but does not state a runtime
  preview/confirm flag because none exists.
- Bilingual CLI docs show a single batch invocation that currently mutates immediately.
- Bilingual safety docs list tag batch as side-effectful but omit its required two-step flow.

## Scope decision

The parent explicitly leaves full MutationPlan/OperationJournal out of scope. This child adds a
command-local in-memory plan/report seam, explicit confirm, ceiling, and complete response-time
outcomes only. It does not promise crash recovery, retry tokens, or remote-version binding.
