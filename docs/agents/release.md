# Release and GitHub Pages contract

Claude Code, Codex, Grok Build, Kimi Code, and OMP share this release contract.

## Tag and Cargo versions

- Future GitHub release tags MUST be `v` plus Cargo `workspace.package.version`: `vX.Y.Z`.
- Cargo `workspace.package.version` stays numeric `X.Y.Z` with no `v` prefix.
- Current Cargo version is `1.0.1`. The current example tag is `v1.0.1`.

Do not change `Cargo.toml` to add a `v` prefix.

## GitHub Pages policy

The `github-pages` environment currently allows:

- branch `main`
- tag pattern `v*`

Do not widen that policy.

`.github/workflows/deploy-docs.yml` runs on a published GitHub release and on manual `workflow_dispatch`. The environment still rejects a tag that does not match `v*`. Do not add a tag-preflight engine. Do not change `deploy-docs.yml`.

## Historical failure

Published tag `1.0.0` (no `v`) was rejected by environment protection. Deploy docs run [29644116256](https://github.com/bahayonghang/zotero-cli/actions/runs/29644116256) at SHA `dfc672bfcea344260c03abbcb5ef213edd116407` built the VitePress site, then failed deploy with:

`Tag "1.0.0" is not allowed to deploy to github-pages due to environment protection rules.`

SHA `dfc672bfcea344260c03abbcb5ef213edd116407` later succeeded through a manual `main` dispatch (run [29644167450](https://github.com/bahayonghang/zotero-cli/actions/runs/29644167450)). The root cause is tag shape, not VitePress.

Leave the existing `1.0.0` tag and GitHub release untouched.

## Authorization stages

Treat these as independent scopes:

1. Local documentation and prep
2. Creating a real tag or GitHub release
3. Hosted GitHub Pages acceptance

Completing local documentation is not authorization to create tags, create releases, or change GitHub environments.

The hosted Deploy-docs path for a real release tag stays **UNVERIFIED** until a separately authorized compliant `vX.Y.Z` release run succeeds. Do not mark hosted PASS from documentation work. A successful `workflow_dispatch` on `main` does not verify the release-tag path.
