<!--
  Keep the PR title short. Use one of the conventional prefixes:
  feat / fix / refactor / docs / test / chore / perf / ci
-->

## Summary

<!--
  One or two bullets:
  - What changed
  - Why it matters for a zot user or AI agent
-->

-
-

## Area(s) touched

- [ ] `src/zot-cli` (binary / CLI surface)
- [ ] `src/zot-core` (config / models / envelope)
- [ ] `src/zot-local` (SQLite, PDF, workspace, local index)
- [ ] `src/zot-remote` (Web API, Better BibTeX, Scite, embeddings)
- [ ] `skills/zot-skills/SKILL.md`
- [ ] `docs/` (Chinese)
- [ ] `docs/en/` (English)
- [ ] `.github/` (workflows / templates)
- [ ] Other:

## Verification

- [ ] `just ci` passes locally (`fmt --check`, `check`, `clippy -D warnings`, `test`).
- [ ] If the CLI surface changed, `docs/cli/**` and `docs/en/cli/**` are updated.
- [ ] If command routing changed, `skills/zot-skills/SKILL.md` stays consistent.
- [ ] If a new env var or config key was added, `README.md`, `README.zh-CN.md`, and `AGENTS.md` are updated.
- [ ] No direct reads or writes to `zotero.sqlite` in new code (writes go through `zot-remote`).

## Notes for reviewers

<!-- Risk, rollout, follow-ups, or anything worth flagging. -->
