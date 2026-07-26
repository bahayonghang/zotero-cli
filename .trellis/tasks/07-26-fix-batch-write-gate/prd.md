# P1: Batch tag write safety gate

## Goal

Make `item tag batch` preview-first and bounded at runtime. A filter must be evaluated locally
before any Web credential/client is required; only explicit `--confirm` may mutate, and every
attempted add/remove operation must be reported even when earlier operations fail.

## Background

- The audit report M-03 (`zotero-cli-code-audit-2026-07-25.md:66-67,121,521-536,772-779`)
  confirms that `src/zot-cli/src/commands/item/tag.rs:41-83` immediately loops over fuzzy search
  results and performs Web writes without preview or confirmation. The first `?` aborts the
  process, hiding already-applied mutations.
- `src/zot-cli/src/cli/args.rs:558-571` currently exposes only filters, mutations, and `--limit`.
  `SearchResult.total` already reports the complete match count while `items` contains the
  deterministic `--limit` selection, so this task can expose both without a new query API.
- `skills/zot/SKILL.md:361-367` classifies tag batch as a high-risk batch write, but the runtime
  does not enforce its preview/confirm policy. Merge, dedupe, import, and status sync already
  establish preview-first command precedent.
- Parent `07-26-audit-remediation/prd.md:28,40,45-50,55` assigns the minimal tag-batch repair to
  this child, orders it after the stable JSON error contract, and explicitly excludes the full
  MutationPlan/OperationJournal architecture.

## Requirements

### R1 Preview-first command surface

- Add `--confirm` and `--max-affected <N>` to `item tag batch`; the maximum defaults to 50 and
  must be positive. Existing `--limit` remains the number of deterministic matches selected for
  this invocation and must also be positive.
- Without `--confirm`, perform only the local search. Do not construct `ZoteroRemote`, require Web
  credentials, or call a writer.
- Preview output must include `state: preview`, full `matched`, selected `affected`, `truncated`,
  at most 10 `sample_keys`, requested added/removed tags, `max_affected`, and
  `exceeds_max_affected`.
- `--confirm` is rejected before writer construction with stable code `batch-tags-max-affected`
  when selected targets exceed the ceiling. The hint must tell the caller to narrow filters or
  explicitly raise the ceiling after reviewing preview.

### R2 Validated mutation request

- Preserve the existing requirement for `--query` and/or `--tag` and for at least one mutation.
- Reject blank filter tags, blank mutation tags, zero limits, and any tag present in both add and
  remove sets using stable `batch-tags-*` `ZotError::InvalidInput` codes.
- Validation and the max gate happen before Web credentials or mutation calls.
- Do not silently raise `--limit`, auto-confirm, or infer permission from the presence of API
  credentials.

### R3 Best-effort apply report

- On confirmed apply, process selected item keys in deterministic preview order. For each key,
  attempt requested add and remove operations independently; record a failed operation and
  continue with the remaining operation and remaining keys.
- Nested failures use the unified `AppError` classifier's canonical `ErrorPayload`; do not expose
  verbose chains or reduce domain errors to unstructured strings.
- Return a structured report with `state` (`applied`, `partial`, or `failed`), match/selection
  summary, operation counts, and per-operation success/failure entries containing key,
  operation, and structured error for failures.
- A partial/all-failed report is still the command result rather than a top-level abort because
  callers need the full remote side-effect ledger. `state` and failure counts are authoritative;
  this is an in-memory response, not a durable or resumable journal.

### R4 Documentation and scope

- Update canonical `skills/zot`, bilingual safety/CLI docs, and README so agents always preview,
  inspect `matched/affected/exceeds_max_affected`, then rerun the same filters/mutations with
  `--confirm`; never describe partial as success.
- Regenerate skill mirrors through the repository recipe and keep mirror checks green.
- Do not add a plan ID/token, persisted ledger, resume/reconcile, version snapshot, generalized
  mutation policy, or changes to other batch/destructive commands.

## Acceptance Criteria

- [ ] CLI parse tests cover `--confirm` and `--max-affected`; validation tests cover empty,
      conflicting, and zero-valued inputs with stable codes.
- [ ] Preview tests prove no writer/credential path is called, distinguish total matches from the
      limited target set, cap samples at 10, and report an exceeded ceiling without mutating.
- [ ] Apply gate tests prove `affected > max_affected` fails before writer calls.
- [ ] Fault-injection tests prove add/remove and later keys continue after failures; reports
      distinguish applied, partial, and failed states and preserve structured domain/generic
      error codes.
- [ ] Canonical skill plus bilingual docs describe preview/confirm/max semantics; generated
      mirrors match; focused tests and final `just ci` pass.

## Out Of Scope

- Full `MutationPlan`, plan token/TTL, remote version snapshot, `OperationJournal`, crash-safe
  resume/reconcile, idempotency ledger, or application/use-case layer.
- Retrofitting every mutation command; this child owns only `item tag batch`.
- Changing local search/trash semantics, which belong to `07-26-fix-db-semantics-perf`.
