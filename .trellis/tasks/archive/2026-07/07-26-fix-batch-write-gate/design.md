# Design: preview-first batch tag executor

## Command flow

```text
ItemTagBatchArgs
  -> validate filters, tags, limit, max-affected
  -> LocalLibrary::search(limit)
  -> BatchTagPlan { total matches, selected keys, preview summary }
       -> no --confirm: CommandOutput(preview), no remote client
       -> --confirm + over ceiling: batch-tags-max-affected, no remote client
       -> --confirm + within ceiling: WebBatchTagWriter
            -> add/remove operation per selected key
            -> capture each outcome and continue
            -> BatchTagReport(applied | partial | failed)
```

The command-specific plan/report types stay private to `tag.rs`. They are a narrow test seam,
not the report's deferred generalized MutationPlan architecture.

## Selection and ceiling contract

- `matched = SearchResult.total`: every local item matching the filters.
- `affected = SearchResult.items.len()`: the ordered subset selected by `--limit`.
- `truncated = matched > affected`.
- `sample_keys = first min(affected, 10)`; samples are display evidence, not an apply token.
- `max_affected` is independent of `limit`. Preview is always allowed and reports
  `exceeds_max_affected`; confirmed apply fails closed when `affected > max_affected`.
- Defaults remain `limit = 50`, `max_affected = 50`, preserving the old target cap while adding
  explicit permission.

The second invocation re-evaluates the local snapshot. It is not cryptographically bound to the
preview; eliminating that TOCTOU requires the excluded versioned plan-token architecture.

## Apply seam and report

```rust
trait BatchTagWriter {
    async fn add_tags(&self, key: &str, tags: &[String]) -> anyhow::Result<()>;
    async fn remove_tags(&self, key: &str, tags: &[String]) -> anyhow::Result<()>;
}
```

Production wraps `ZoteroRemote`; tests inject a deterministic fake. Each non-empty add/remove
set is one operation per key. A failed add does not suppress remove for that key, and neither
failure suppresses later keys.

The report carries:

- summary: `state`, `matched`, `affected`, `truncated`, `sample_keys`, tags, ceiling flags;
- counts: `attempted_operations`, `succeeded_operations`, `failed_operations`;
- outcomes: `{ key, operation }` successes and
  `{ key, operation, error: ErrorPayload }` failures.

State is computed after the loop: no failures is `applied`; mixed outcomes is `partial`; no
successful operations with at least one failure is `failed`. Zero selected targets is `applied`
with zero counts.

## Validation and errors

| Condition | Code | Side effects |
|---|---|---|
| no query and no tag | `batch-tags-filter` | none |
| blank filter tag | `batch-tags-filter` | none |
| no add/remove tags | `batch-tags-op` | none |
| blank mutation tag | `batch-tags-op` | none |
| same tag in add/remove | `batch-tags-conflict` | none |
| `limit == 0` | `batch-tags-limit` | none |
| `max_affected == 0` | `batch-tags-max-affected` | none |
| confirmed target count over ceiling | `batch-tags-max-affected` | none |
| per-operation remote/generic error | nested canonical error payload | continue |

## Compatibility, docs, and rollback

Existing invocations become preview-only, an intentional safety change. Existing filter and
mutation flag names remain stable. Human output remains pretty JSON because the report is
multi-dimensional; JSON mode wraps the same typed report in the standard envelope.

Canonical skill/docs must show a preview invocation and a second identical invocation with
`--confirm`; larger batches must additionally raise `--max-affected`. Roll back CLI, tests,
canonical docs, generated mirrors, and specs together; never retain docs that imply a gate the
runtime no longer enforces.
