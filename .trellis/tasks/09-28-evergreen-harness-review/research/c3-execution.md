# C3 execution record

Date: 2026-09-28. Scope: approved C3 harness contract refresh.
Base HEAD: `ba4e10e987e7afe65dccb8450445d41bc9e08d79`. The changes remain
uncommitted; that HEAD does not include the C3 patch.

## Changed project files

- `AGENTS.md`: short platform IDs, renderer proof limits, Grok/Kimi pull-context
  requirements, explicit Codex inline policy, approval/model boundaries.
- `.trellis/config.yaml`: explicit `codex.dispatch_mode: inline`.
- `docs/agents/harnesses.md`: dated five-tool evidence matrix, native routes,
  permission boundaries, strong-model review and fixed-scope execution allocation,
  model-selection limits, official sources, and tool-scoped durable writeback.
- `README.md` and `README.zh-CN.md`: duplicate merges use gated Web API writes;
  Desktop connector imports new BibTeX/RIS records only.

Root `CLAUDE.md` remains `@AGENTS.md`. Canonical skills, ignored native adapters,
and `.trellis/.template-hashes.json` were not edited. C4 schedule documentation
is deferred to the parent session to avoid concurrent writes to the matrix.

## Verification

| Check | Result | Evidence |
| --- | --- | --- |
| `python -B -m unittest scripts.tests.test_harness_platform_routing -v` | PASS: 10 tests, 3.953 seconds | C3 execution tool output; ran after inline config was written |
| Actual Codex Phase 2.1 rendering | PASS: inline implementation steps; no spawn instruction | `c3-codex-2.1.log` |
| Actual Codex Phase 2.2 rendering | PASS: inline quality checks; no spawn instruction | `c3-codex-2.2.log` |
| Effective platform resolver | PASS: `codex-inline` | `c3-verification.json` |
| Scoped `git diff --check` | PASS | `c3-scoped-diff-check.log` |
| Protected tracked files against HEAD | PASS | `c3-protected-files.log` |
| Matrix dates/versions, no archive-sensitive task links | PASS | assertions in `c3-verify.py` |
| `just ci` after all approved changes | Pending parent final gate | Not run by C3 agent |
| VitePress build after all approved changes | Pending parent final gate | Not run by C3 agent |

`c3-verify.py` records working-tree hashes and the command results in
`c3-verification.json`. The parent will add C4 content to the shared matrix;
the recorded matrix hash describes the completed C3 state before that edit.

## Acceptance and remaining boundary

- C3-AC1: implemented; shared instructions agree with C2 aliases and explicit
  inline config. Real repository Phase 2.1 and 2.2 text was checked semantically.
- C3-AC2: implemented; all five tools have official, repository, local, and
  unverified fields. No lower-cost Kimi routing or sandbox enforcement is claimed.
- C3-AC3: implemented; the bilingual first-screen text agrees with
  `selected_merge_writer` / `WebMergeWriter` and the canonical operator skill.
- C3-AC4: focused checks pass. The parent retains final strong-model review,
  `just ci`, and the docs build before marking the acceptance criterion complete.

The C1/C2/C3 agent dispatches were authorized under the prior `auto` setting.
After the explicit inline setting, subsequent implementation and review follow
the main-session route. The existing C3 child completed its assigned work directly.

Five-client fresh-start, hook, selected-model, provider, and OS sandbox trials
remain unverified. No paid sessions, global config, library writes, commits,
remote publication, task-state changes, or archive operations were performed.
After `trellis update`, rerun the routing test and verify the local managed
alias patch, native entrypoint IDs, and explicit Codex setting.
