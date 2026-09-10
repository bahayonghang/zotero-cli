# Issue tracker

## Local Trellis versus GitHub Issues

Local Trellis `prd.md`, `design.md`, and `implement.md` under `.trellis/tasks/`
are the implementation-acceptance source for an active task.

GitHub Issues for `bahayonghang/zotero-cli` are authorized collaboration and
remote tracking records. GitHub Issues are not the implementation-acceptance
source for an active Trellis task.

Do not create, comment on, or close GitHub issues without explicit
authorization.

## When GitHub operations are authorized

Use the `gh` CLI for authorized remote operations. Infer the repo from
`git remote -v` — `gh` does this automatically when run inside a clone.

- **Create an issue**: `gh issue create --title "..." --body "..."`. Use a heredoc for multi-line bodies.
- **Read an issue**: `gh issue view <number> --comments`, filtering comments by `jq` and also fetching labels.
- **List issues**: `gh issue list --state open --json number,title,body,labels,comments --jq '[.[] | {number, title, body, labels: [.labels[].name], comments: [.comments[].body]}]'` with appropriate `--label` and `--state` filters.
- **Comment on an issue**: `gh issue comment <number> --body "..."`
- **Apply / remove labels**: `gh issue edit <number> --add-label "..."` / `--remove-label "..."`
- **Close**: `gh issue close <number> --comment "..."`

## When a skill says "publish to the issue tracker"

Create a GitHub issue only after explicit authorization.

## When a skill says "fetch the relevant ticket"

Run `gh issue view <number> --comments`.
