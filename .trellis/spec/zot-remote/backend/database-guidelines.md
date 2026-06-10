# Database Guidelines

`zot-remote` does not own local persistence. It calls remote HTTP services and
returns typed results to `zot-cli`.

## Persistence Boundary

- Do not add SQLite, migrations, cache files, or workspace files to this crate.
- Local Zotero reads and sidecar indexes belong in `zot-local`.
- Config loading belongs in `zot-core` and `zot-cli::AppContext`.
- Remote mutations are expressed as Web API requests, not direct database
  changes.

## Remote Write Patterns

- Zotero object updates fetch the current object first and send
  `If-Unmodified-Since-Version` with the fetched version. Examples:
  `update_item_fields`, `delete_item`, `restore_item`, note updates,
  collection updates, and flat item updates in `zotero.rs`.
- Create endpoints use `Zotero-Write-Token` with a UUID where Zotero supports
  idempotency.
- Saved search deletion fetches library version via `library_version()` and
  sends it as a precondition.
- Attachment upload is multi-step: create attachment item, authorize upload,
  upload bytes to Zotero's provided URL, then register the upload key.

## Code Example

Version-preconditioned writes should follow the existing shape:

```rust
let item = self.get_item_data(key).await?;
let response = self
    .http_put(self.endpoint(&format!("items/{key}")))
    .header("If-Unmodified-Since-Version", item.version().to_string())
    .json(&item.data)
    .send()
    .await
    .map_err(remote_err("update-item"))?;
self.ensure_empty(response, "update-item").await
```

## Avoid

- Never implement library mutations by touching `zotero.sqlite`.
- Do not silently ignore write precondition failures; `http_hint` maps 412/428
  style statuses to retry guidance.
- Do not persist remote response data in this crate. If a cache is needed, put
  it behind a local store in `zot-local` or a CLI-level explicit file output.
