# Implementation Plan

1. Add workspace `quick-xml` and `zot-remote` inheritance; add the fixed API-version header to
   `ZoteroRemote` and header assertions to existing fake-server tests.
2. Implement shared request classification, bounded retry/backoff/`Retry-After`, and bounded
   sanitized error-body reading in `http.rs`; migrate all remote GETs and Zotero write-token
   creates, leaving other mutations single-shot.
3. Add retry/error regressions with the recognized loopback fake server, including GET 429/5xx,
   stable write token, non-retried conditional write, max attempts, and body truncation/control
   characters.
4. Add the untrusted PDF downloader in `zot-remote`: URL/authority validation, public IP policy,
   DNS resolution, manual redirect loop, content-length/type/magic checks, and bounded chunked
   writing. Export only the narrow production API needed by `zot-cli`.
5. Replace CLI `.bytes()`/manual temp path with `NamedTempFile` plus the guarded downloader; add
   focused pure/integration tests proving rejection prevents upload and temp ownership is scoped.
6. Refactor attachment authorization to stream MD5, enforce regular-file/100 MiB bounds before
   create, construct at most one full upload buffer, and wrap post-create failures with hard-delete
   cleanup evidence. Extend fake-server scripts for authorize/upload/register/cleanup failures.
7. Replace arXiv Atom field regexes with a `quick-xml` reader and add namespace, entity, CDATA,
   nested text, multiple-author, missing-entry, and malformed fixtures.
8. Run focused checks in order:
   - `cargo fmt --all --check`
   - `cargo test -p zot-remote`
   - `cargo test -p zot-cli`
   - `cargo test -p zot-cli --test workspace_version_guard`
   - `cargo clippy -p zot-remote -p zot-cli --all-targets -- -D warnings`
9. Load `trellis-check`, review request eligibility, redirect/IP handling, cleanup evidence,
   credential headers, temp cleanup, and XML error paths; fix findings and run `just ci`.
10. Load `trellis-update-spec` to record the reusable retry, bounded error, untrusted download,
    attachment compensation, and Atom parser contracts before the task commit/archive.

## Risk And Rollback Points

- Retry migration is call-site-sensitive: a missed GET remains brittle, while a mistakenly
  migrated non-token mutation can duplicate writes. Search every `.send()` and assert the final
  matrix before implementation commit.
- DNS validation must inspect every returned address and every redirect; accepting one public
  address cannot override a forbidden sibling address.
- Test-only loopback download allowances must be structurally unreachable from production APIs.
- Compensation must never delete the parent item or an attachment from a prior invocation; it
  receives only the newly returned attachment key.
- No task outside `07-26-fix-remote-http-hardening` and no root audit report may be staged with
  this task's planning or implementation commits.
