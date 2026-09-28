# C2 execution report

Date: 2026-09-28. Task: `09-28-harness-platform-routing`.

## Change

- `.trellis/scripts/common/workflow_phase.py:135-145`: map `claude` to `claudecode`, `kimi` to `kimicode`, and `omp` to `ohmypi` after existing case/separator normalization. Apply the same map to the requested platform and marker names. Tracked diff: 4 added lines, 1 removed line.
- `scripts/tests/test_harness_platform_routing.py`: add 10 test methods under existing unittest discovery. No new dependency or test framework.
- Do not change Codex dispatch resolution, workflow text, adapters, configuration, template hashes, task state, or source outside the assigned files. The repository has no `packages/cli/src/templates/trellis/scripts` tree to synchronize.

## Validation

| Command / probe | Result | Evidence |
| --- | --- | --- |
| `python -m unittest discover -s scripts/tests -p test_harness_platform_routing.py` before patch | Exit 1; 10 methods, 20 failing subtests | `c2-tests-red.log`, `c2-tests-red.json` |
| Same focused command after patch | Exit 0; 10 methods passed | `c2-tests-green.log`, `c2-test-results.json` |
| `python -m unittest discover -s scripts/tests -p test_*.py` | Exit 0; 23 methods passed | `c2-tests-all.log`, `c2-test-results.json` |
| Scoped `git diff --check` | Exit 0; only Git LF/CRLF information | `c2-diff-check.log` |
| Real repository CLI: five short platform IDs, steps 2.1 and 2.2 | All 10 invocations exit 0 and contain the required implementation/check body | `c2-platform-probe.json`, `c2-phase-*.txt` |

The red run failed on six alias subtests, two filter subtests, six five-platform CLI subtests, and six short-ID CLI subtests. Steps returned exit 0 while the required body was absent. Step 2.2 retained common final-pass text; the regression therefore checks role and action clauses instead of output length.

The tests cover case/space/hyphen/underscore equivalence in both directions; OMP stays distinct from Pi. Grok, Cursor, unknown IDs, and the Antigravity inline branch retain existing behavior. Codex coverage includes missing configuration, an empty mapping, auto, legacy sub-agent, explicit inline, invalid mode, and invalid section shape. CLI configuration cases run in temporary directories using the actual repository workflow and actual `get_context.py` entrypoint. A separate test checks the current repository's configured Codex mode, so the approved C3 inline setting does not invalidate the suite.

The direct probe recorded unchanged SHA-256 values before/after for both changed source files, configuration, workflow, and C2 task metadata. Actual step 2.1 bodies have 851 characters for Claude/Codex/OMP and 843 for Grok/Kimi; all step 2.2 bodies have 1074 characters at this pre-C3 configuration. These lengths are descriptive evidence; tests require meaningful clauses.

## Acceptance status and handoff

- C2-AC1: passed for aliases, five platforms, and both steps.
- C2-AC2: passed for Codex modes and unaffected platform separation.
- C2-AC3: red/green and all Python discovery passed. Parent must record `just ci` and final combined review.
- C3 writeback: record exact IDs `claude`, `codex`, `grok`, `kimi`, `omp`; corresponding display names; the local alias patch; the regression command; and the need to repeat the check after a Trellis update.
- Scope of evidence: local renderer and configuration behavior. Fresh native client sessions, hook execution, model selection, and OS isolation remain unverified.
- No commit, archive, remote operation, global configuration change, or task-state change was performed by C2.
