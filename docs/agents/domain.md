# Domain Docs

How the engineering skills should consume this repo's domain documentation when exploring the codebase.

## Layout

This repo uses the single-context layout:

- `CONTEXT.md` at the repo root for project vocabulary and domain language
- `docs/adr/` at the repo root for architectural decision records

These files do not have to exist yet. If they are absent, proceed silently. The producer skill (`/grill-with-docs`) creates them lazily when terms or decisions actually get resolved.

## Before exploring, read these

- `CONTEXT.md` at the repo root, if it exists
- ADRs under `docs/adr/` that touch the area you're about to work in, if any exist

## Use the glossary's vocabulary

When your output names a domain concept in an issue title, refactor proposal, hypothesis, or test name, use the term as defined in `CONTEXT.md`. Do not drift to synonyms the glossary explicitly avoids.

If the concept you need is not in the glossary yet, either reconsider the language or note the gap for `/grill-with-docs`.

## Flag ADR conflicts

If your output contradicts an existing ADR, surface it explicitly rather than silently overriding:

> _Contradicts ADR-0007 — but worth reopening because..._
