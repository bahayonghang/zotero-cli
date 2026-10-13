# Local content operations (Zotero 10+)

`item update` and collection membership operations support the official Local API. Keep Zotero running and enable local communication in Advanced settings. No Web API key or extra plugin is required. Zotero asks for authorization when a confirmed write needs it.

Run `zot --json doctor` first. `local_api_write.supported` describes protocol support; `credential_present` only describes a cached authorization. Doctor never prompts or attempts a write, and reports verified=false.

## Preview and confirm

Replace these example eight-character item/collection keys with real keys:

```bash
zot --json item update ITEMABCD --write-backend local --field DOI=10.1000/example
zot --json collection add-item COLLB234 ITEMABCD --write-backend local
zot --json collection remove-item COLLA234 ITEMABCD --write-backend local
zot --json collection move-item ITEMABCD --from COLLA234 --to COLLB234 --write-backend local
```

These return preview, or unchanged when no change is needed. Inspect before/after/skipped_fields, then repeat the same arguments with `--confirm --expected-server-id <server_id> --expected-version <expected_version>` from that preview. Never substitute Web/SQLite versions. Preview again on a conflict.

A move removes the source membership and adds the destination, preserving all other memberships and the item identity. It does not move files or copy items. Only top-level bibliographic items in the same library are supported. Targets come from arguments, not UI selection. move-item requires an explicit local backend.

## Fields and creators

Fields are validated against the current itemType/schema. Empty fields are filled by default. Include `--overwrite` in both preview and confirmation to replace specified nonempty values. Clearing fields and changing itemType are outside this feature.

Use `--creators-file authors.json` for a complete ordered creator list:

```json
[
  {"creatorType":"author","firstName":"Ada","lastName":"Lovelace"},
  {"creatorType":"author","name":"Research Institute"}
]
```

Replacing existing creators requires --overwrite. Names are not split, appended, or deduplicated automatically. Native citationKey can be explicitly edited when supported by the schema; it is distinct from the item key. Better BibTeX regeneration is not guaranteed by this command.

## Results and boundaries

- applied + verified=true: the write response and readback both confirm the requested state.
- unknown: submission outcome is unknown. Even observed_expected_state=true cannot attribute the state to this request; do not blindly retry.
- verification-failed: write_accepted=true, but readback failed or differs.
- Conflicts, denied authorization, permissions, and invalid input use the standard error envelope.

Single-use authorization stays in memory. Only remembered authorizations are saved by profile and Server ID in local-api-credentials.json under the platform configuration directory (AppConfig::config_dir(); typically `%AppData%\zot` on Windows). Keys are excluded from config show and diagnostics.

Existing commands still default to Web. Local-only options are rejected on Web, and local failures never switch transports. Tags, notes, attachments, batch operations, and online enrichment are not covered by the local writer. SQLite remains read-only.
