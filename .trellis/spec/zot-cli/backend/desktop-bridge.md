# Desktop Bridge Contract

## Scenario: Authenticated Zotero Desktop Bridge

### 1. Scope / Trigger

Use this contract when changing `plugins/zot-bridge/`, `src/zot-desktop`, the
`zot bridge` command family, bridge configuration, or doctor capability output.
The bridge is a loopback-only, allowlisted write transport. It must never become
a generic script, eval, or arbitrary field-update surface.

### 2. Signatures

CLI surface:

```text
zot bridge setup [--output <dir>]
zot bridge pair <code>
zot bridge status
zot bridge revoke [--local-only]
```

Protocol v1 surface, all JSON POST requests:

```text
http://127.0.0.1:23119/zot-bridge/v1/health
http://127.0.0.1:23119/zot-bridge/v1/pair
http://127.0.0.1:23119/zot-bridge/v1/status
http://127.0.0.1:23119/zot-bridge/v1/auth/revoke
http://127.0.0.1:23119/zot-bridge/v1/merge/preview
http://127.0.0.1:23119/zot-bridge/v1/merge/apply
```

`src/zot-desktop::DesktopClient` owns HTTP and DTO decoding. `zot-cli` owns
configuration mutation and output. The plugin owns Zotero lifecycle, auth, and
endpoint registration. `zot-local` and `zot-remote` must not depend on this
transport.

### 3. Contracts

Every request contains `protocol_version`, `request_id`, `sent_at`, and
`client { name, version }`. Protected requests use `Authorization: Bearer`.
Responses use the standard `ok/data/meta` or `ok/error/meta` shape; response
`request_id` and `protocol_version` must match the request and client.

Pairing codes are eight-character CSPRNG values, live in plugin memory for five
minutes, and are single-use. Only Zotero UI actions may create or reveal a code.
`/pair` validates `ZOT_BRIDGE.pairing`; it must never call
`ensurePairingCode()` to create an undisplayed replacement. The plugin stores
only the token SHA-256 hash; CLI config stores the token and never emits it.

Config defaults to `write_backend = "web"` for old files. Successful pairing
sets the selected config target to `desktop`. Target precedence is explicit
profile, default profile, then root. A command-line backend override is not
persisted.

The plugin persists both the bearer-token hash and a random profile-bound
`instance_id` in Zotero preferences. Health, pair, and status responses include
that ID; CLI config stores it beside the token but only exposes an eight-character
`connection_id` label. Installing or upgrading the same plugin ID in the same
Zotero profile reuses both preferences, so `bridge setup` must direct an already
configured target to `bridge status`, not to pairing. Revocation clears the token
hash but retains the instance identity.

Before any configured status, revoke, merge preview, or merge apply sends a
bearer token, the CLI probes health and compares the configured and observed
instance IDs. A non-empty mismatch fails without exposing either ID. Empty IDs
remain compatible with bridge/config versions that predate this field; after a
successful `bridge status`, a legacy config stores the observed ID without
changing its selected write backend. A different Zotero profile never receives
copied credentials and must be explicitly paired once.

Doctor reports `local_sqlite_read`, `local_http_read`, `desktop_write`, and
`web_write` independently. Disabling the plugin may make `desktop_write`
unavailable while Local HTTP and SQLite remain available.

Merge preview accepts only `scope {type:user|group, group_id?}`, `keeper_key`,
and `source_keys` in addition to the base request. It validates an editable
library and current top-level items, computes metadata fill plus a canonical
item/child fingerprint, and returns a two-minute opaque `plan_token`. The Rust
client keeps that token private; CLI output exposes only a one-way `plan_id`.

Merge apply accepts only `plan_token` and `operation_id`. The plugin binds the
plan to the bearer authorization, scope, keys, fingerprint, and preview. It
sets only planned keeper fields in memory, then calls Zotero 9's native
`mergeItems()`; it never saves metadata separately. A timeout retry reuses the
same request ID, plan token, and operation ID. A later request with the same
operation ID returns the cached result with `already_applied: true` and does
not run native merge twice.

### 4. Validation & Error Matrix

| Condition | Required result |
| --- | --- |
| Missing, used, or expired displayed code | `401 bridge-pair-expired` |
| Active displayed code does not match | `401 bridge-pair-code` |
| Missing or wrong bearer token | `401 bridge-auth` |
| Configured and running non-empty instance IDs differ | client `bridge-profile-mismatch` before protected request |
| Legacy config has no instance ID | allow authenticated status, then persist observed ID only |
| Browser `Origin` present | reject before operation (`bridge-origin` in handler tests) |
| Host outside loopback allowlist | reject before operation (`bridge-host` in handler tests) |
| Unknown JSON field | `400 bridge-unknown-field` |
| Reused request ID with different payload/auth | `409 bridge-replay` |
| Protocol mismatch | `409 bridge-protocol` or client `bridge-protocol` |
| Endpoint absent while Zotero Local HTTP responds | `bridge-not-installed` |
| Invalid success JSON | client `bridge-invalid-response` |
| Response over 64 KiB | client `bridge-response-too-large` |
| Missing/expired merge plan | `409 bridge-plan-expired` |
| Item, version, field, relation, or direct-child drift | `409 bridge-item-changed` |
| Read-only group/library | `403 bridge-library-readonly` |
| Child, attachment, note, or annotation candidate | `400 bridge-invalid-child` |
| Candidate resolves outside selected library | `400 bridge-cross-library` |
| Native merge throws | `500 bridge-transaction`; DB rollback plus object-cache reload |

### 5. Good / Base / Bad Cases

- Good: UI shows a code, `bridge pair` exchanges it once, `bridge status`
  authenticates, and doctor reports an editable desktop library.
- Upgrade good: reinstall the XPI into the same Zotero profile, restart, and
  `bridge status` reuses authorization while reporting the same connection ID.
- Base: no token is configured; health still identifies the plugin and status
  reports installed but not paired with a recovery hint.
- Migration base: a pre-instance-ID config authenticates once and records the
  running profile identity without changing `write_backend`.
- Bad: a browser request, remote host, stale code, wrong token, replay conflict,
  incompatible protocol, or arbitrary field is rejected without mutation.
- Profile bad: starting a different Zotero profile returns
  `bridge-profile-mismatch`; the CLI does not send its configured token or
  silently transfer authorization.
- Merge good: preview emits a redacted plan ID, apply fills only compatible
  empty keeper fields, and native merge owns children/relations/trash.
- Merge base: preview only; the plugin stores a short-lived plan but the
  library is unchanged.
- Merge bad: drift, expired plans, read-only/cross-library/child candidates,
  or a reused operation ID with different data fails before mutation.

### 6. Tests Required

- `cargo test -p zot-desktop`: health/pair/status/revoke, bearer capture, 401,
  timeout, invalid JSON, oversize, replayed response, request ID mismatch, and
  protocol mismatch; merge DTO shape, plan-token Debug redaction, and timeout
  retry with byte-identical idempotency payload.
- `node --test plugins/zot-bridge/tests/bootstrap.test.cjs`: Origin/unknown
  fields, five-minute expiry state, single use, replay conflict/cache, revoke,
  merge fill/skip mapping, plan expiry, fingerprint drift, read-only,
  cross-library, child rejection, native transaction failure, operation replay,
  and shutdown endpoint removal.
- `just xpi-check`: allowlisted assets plus manifest/plugin/workspace versions.
- CLI/config tests: parse surface, target precedence, migration default, secret
  redaction, setup's configured/unconfigured next steps, instance-ID migration
  without backend mutation, profile mismatch before protected calls, doctor
  independence, and stable hints.
- Plugin reload test: use one shared preference store across two plugin loads;
  assert the token remains authorized and `instance_id` remains stable. Revoke
  must remove the token hash while retaining the instance ID.
- Real Zotero smoke: install/restart, pair/status, 306-second expiry, revoke and
  re-pair, wrong token, plugin disable -> endpoint 404 while Local HTTP remains
  available, re-enable, and final doctor status.

### 7. Wrong vs Correct

Wrong: generate a replacement inside `/pair`; the client can never know that
hidden code and the expired branch becomes unreachable.

```javascript
const pairing = await this.ensurePairingCode();
```

Correct: pairing creation belongs to Zotero UI startup/menu paths; `/pair`
only consumes the currently displayed in-memory record.

```javascript
const pairing = this.pairing;
if (!pairing || pairing.used || Date.now() > pairing.expiresAt) {
  throw this.error(401, "bridge-pair-expired", "Pairing code expired", hint);
}
```

Wrong: expose the raw plan token in `MergePreview`, Debug, logs, or the CLI
JSON envelope. Correct: keep it in `DesktopMergePreview`/the private writer
handle, redact Debug, and emit only `plan_id` until immediate confirm/apply.

Wrong: make every `bridge setup` tell an already configured user to show a new
pairing code, or copy a bearer token into a newly created Zotero profile.
Correct: same-profile setup says install/restart/status; a non-empty instance-ID
mismatch fails with `bridge-profile-mismatch` and requires one explicit pair.
