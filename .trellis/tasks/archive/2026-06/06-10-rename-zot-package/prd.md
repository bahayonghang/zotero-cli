# Rename zot skill package

## Goal

Rename the bundled agent skill currently published as `zot-skills` so the public skill name no longer ends with `skill` / `skills`, and keep the bundled skill package plus docs references consistent.

User value: the installable agent workflow has a cleaner name while remaining clearly tied to the `zot` CLI and Zotero workflows.

## Confirmed Facts

- The bundled skill currently lives at `skills/zot-skills/`.
- The skill frontmatter is `name: zot-skills`, and the H1 is `# zot-skills`.
- The user selected the new public name `zot`.
- The skill package includes:
  - `skills/zot-skills/SKILL.md`
  - `skills/zot-skills/test-prompts.json`
  - `skills/zot-skills/evals/evals.json`
- Eval metadata currently uses `"skill_name": "zot-skills"`.
- Active references to `zot-skills` and `skills/zot-skills/...` exist in:
  - `AGENTS.md`
  - `README.md`
  - `README.zh-CN.md`
  - `docs/index.md`
  - `docs/en/index.md`
  - `docs/guide/*`
  - `docs/en/guide/*`
  - `docs/skills/*`
  - `docs/en/skills/*`
- `just ci` is the main Rust verification gate.
- Docs are VitePress-based under `docs/`, with `npm --prefix docs run build` as the build script.

## Requirements

- Rename the skill package directory from `skills/zot-skills/` to `skills/zot/`.
- Update the skill's public metadata and visible heading to `zot`.
- Update bundled eval metadata so it matches the new skill name.
- Update installation commands and internal links in README and docs to the new package path/name.
- Update prose references so they refer to the renamed skill without leaving stale `zot-skills` wording in active docs.
- Keep the runtime `zot` CLI behavior unchanged.
- Preserve unrelated working-tree changes, including the existing untracked `TODO.md`.

## Acceptance Criteria

- [x] The old public skill name `zot-skills` no longer appears in active skill metadata, README, docs, eval metadata, or install commands.
- [x] The old package path `skills/zot-skills` no longer appears in active repository guidance or docs.
- [x] The renamed `skills/zot/` package contains the same functional files: `SKILL.md`, `test-prompts.json`, and `evals/evals.json`.
- [x] The skill frontmatter is `name: zot`, the H1 is `# zot`, and eval metadata uses `"skill_name": "zot"`.
- [x] Install commands use `--skill zot`.
- [x] README and VitePress docs link to the renamed package path.
- [x] `just ci` passes.
- [x] `npm --prefix docs run build` passes, or any failure is documented as pre-existing / environment-related.

## Out Of Scope

- Renaming the Rust workspace, crates, binary, or CLI commands.
- Changing Zotero workflow routing, write-safety rules, or `zot` runtime behavior.
- Rewriting the skill content beyond name/path/reference consistency.
- Removing unrelated dead code or generated files.

## Decisions

- The new public skill name is `zot`, using package path `skills/zot/`, frontmatter `name: zot`, H1 `# zot`, and install command `--skill zot`.

## Open Questions

- None.

## Notes

- This appears to be a lightweight PRD-only task unless the chosen name requires compatibility aliases or package-migration behavior.
