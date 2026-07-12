# Merge & Dedupe Engine

Executable contracts for the shared merge engine (`item merge`,
`library duplicates-merge`) and the batch cleanup command (`library dedupe`).
Introduced by task `07-11-dedupe-cleanup`; evidence and sources live in that
task's `research/2026-07-11-feasibility.md`.

## Scenario: cross-type safe merge + batch dedupe

### 1. Scope / Trigger

New command signature (`library dedupe`), cross-layer response contract
changes (`MergePreview` / `DedupePlan`), and a remote write path — code-spec
depth is mandatory.

### 2. Signatures

```rust
// zot-cli/src/commands/item/merge.rs — shared engine
trait MergeWriter {
    preview(keeper_key, source_keys) -> PreparedMerge;
    apply(prepared, operation_id) -> MergeApplyResult;
}
async fn merge_item_set(writer: &dyn MergeWriter, keeper_key, source_keys, confirm)
    -> Result<MergeOperation>;
fn build_merge_execution_plan(keeper, sources, keeper_children, source_children,
                              source_uris) -> Result<MergeExecutionPlan>; // pure, zero IO

// zot-cli/src/commands/library_dedupe.rs
fn build_dedupe_plan(..., write_backend, include_low_confidence) -> Result<DedupePlan>;
async fn apply_dedupe_plan(merger: &impl GroupMerger, ...) -> DedupeApplyReport;

// zot-remote/src/zotero.rs
pub fn item_uri(&self, key: &str) -> String; // http://zotero.org/users|groups/{id}/items/{KEY}
```

CLI: `zot library dedupe [--method both|doi|title] [--collection <id>] [--limit N]
[--confirm] [--include-low-confidence]`
— dry-run by default; dry-run is pure local and must not construct the remote
or desktop writer (selected writer construction occurs only in `--confirm`).

### 3. Contracts

- `MergePreview` / `MergeApplyResult` carry `skipped_incompatible_fields:
[{field, source_key}]` and `relations_to_add: [uri]`. JSON envelope grows
  additively only.
- Merge/dedupe preview and apply models carry lower-case `write_backend`.
  Desktop preview may add a redacted `plan_id`; raw plan tokens never enter
  `zot-core`. Apply adds `already_applied` (false for Web).
- The keeper PATCH payload **is** `plan.merged_keeper`; `sanitize_flat_item_value`
  strips only `key`/`version`, so `relations` rides through unchanged. Tests
  may therefore assert on the plan object as the wire payload.
- `DedupePlan.groups[]`: `match_type` (may be a combined value like
  `"doi+title"` after component merging), `confidence` (`normal`|`low` with
  `confidence_note`: year spread > 1 or ≥ 2 distinct non-empty DOIs),
  `keeper{key,item_type,title}`, `reason` (auditable, stops at the first
  decisive layer), `absorb[]`. Plus `total_groups`, `confirm_required`.
- `DedupePlan` also records `include_low_confidence`.
- `DedupeApplyReport`: `applied[]`, `failed[{keeper, sources, error}]`,
  `skipped_low_confidence[]`, `total_groups`, `eligible_groups`,
  `applied_groups`, `failed_groups`, and `skipped_low_confidence_groups`.
- Backend selection is exact. Desktop configuration/auth/protocol/transaction
  failures remain desktop errors and never construct or call a Web writer.

### 4. Validation & Error Matrix

| Condition                                                 | Behavior                                                                                                     |
| --------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------ |
| Source field key **absent** on keeper flat JSON           | never filled; recorded in `skipped_incompatible_fields` (prevents Web API 400 "invalid field for item type") |
| Source key missing from `source_uris` map                 | hard error — never silently drop a citation redirect                                                         |
| attachment / note / annotation offered as merge candidate | `ZotError::InvalidInput` (`item-merge`)                                                                      |
| One group fails during `--confirm` apply                  | recorded in `failed[]` as `code: message`; loop continues                                                    |
| Low-confidence group without include flag                | copied to `skipped_low_confidence[]`; writer is never called                                                |
| `--include-low-confidence` with confirm                   | low and normal groups enter the same serial writer loop                                                     |
| Desktop selected but unpaired                             | `bridge-unpaired`; Web credentials are not consulted                                                        |

### 5. Good / Base / Bad Cases (field fill decision)

- Good: keeper key exists and is empty (`""`/null/empty array) → fill from source.
- Base: keeper key exists with a value → keep keeper's value, no fill.
- Bad (forbidden): keeper key absent → filling it sends a type-invalid field
  and the whole PATCH 400s.
- Desktop good: plugin runtime base-field mapping fills a compatible empty
  keeper field, then native `mergeItems()` owns structural merge in one
  transaction.
- Desktop base: preview signs the current candidate fingerprint and does not
  write the library.
- Desktop bad: plugin drift or transaction failure returns a bridge error;
  CLI does not fall back to Web.

The Bad case is safe only because Web API `GET /items/{key}` returns the
**complete field template** for the item's type with `""` for unset fields
(verified against api.zotero.org and cross-checked with `/itemTypeFields`,
2026-07-11). If that upstream behavior ever changes, fall back to filtering
against `GET /itemTypeFields?itemType=X`.

### 6. Tests Required

- merge.rs plan tests: `merged_keeper["relations"]["dc:replaces"]` contains
  every source URI (user- and group-scope prefixes); skipped-field recording;
  relations union dedupes and re-running is idempotent.
- library_dedupe.rs: all four keeper tie-break layers (type rank → non-empty
  field count → attachment count → earlier dateAdded, key as final fallback)
  each assert the exact `reason` string; both `confidence: low` triggers plus
  a normal control; overlap components (partial `{A,B}+{B,C}`, full, and a
  bridging third group folding two components); apply-loop continuation past
  an injected single-group failure via a fake `GroupMerger`.
- Writer conformance: empty sources fail before preview, candidate keys pass
  unchanged, preview never calls apply, and results expose the writer backend.
- Web writer loopback regression asserts the legacy seven-request order,
  version preconditions, keeper `dc:replaces`, and source `{deleted:1}`.
- Dedupe gate tests assert default low groups produce zero writer calls and
  explicit include applies them; counts/lists must agree.
- Desktop protocol tests are owned by `desktop-bridge.md` and must stay green
  with `just xpi-check`.

### 7. Wrong vs Correct

```rust
// Wrong — fills keys the keeper's type does not have → Web API 400:
if target_value.is_none_or(is_empty) { fill(...) }

// Correct — three-way branch on key presence:
match keeper.get(field) {
    None => record_skipped(field, source_key),      // type-incompatible
    Some(v) if is_empty(v) => fill(...),            // legal and empty
    Some(_) => {}                                   // keeper wins
}
```

```rust
// Wrong — a desktop failure silently changes trust boundary:
desktop.preview(...).await.or_else(|_| web.preview(...).await)

// Correct — selected backend owns success and failure for the entire call:
match ctx.write_backend() {
    WriteBackend::Desktop => DesktopMergeWriter::new(...),
    WriteBackend::Web => WebMergeWriter::new(...),
}
```

## Invariants (all three entry points)

- Removal is always merge-semantics + trash (`deleted: 1`, recoverable);
  never HTTP `DELETE` (permanent, cascades into child items).
- The keeper gains a `dc:replaces` relation per absorbed item. This is the
  citation-protection contract: Word/LibreOffice plugins resolve replaced
  items through it (official: dstillman, forums/78483). Deleting a cited item
  without it orphans the citation permanently.
- Zotero's "Merged items must all be of the same item type" is **UI-only**
  (`duplicatesMergePane.js`); the data model allows cross-type merges. The
  engine deliberately supports them — the keeper keeps its own item type.
- A dedupe plan never lists the same item key twice: detection groups sharing
  any key are folded into one connected component before scoring. Without
  this, group B's keeper may already be trashed by group A's apply and absorb
  items into an invisible (trashed) keeper.
- Detection input excludes trashed items (`SearchOptions::exclude_trashed`,
  see zot-local database guidelines).
