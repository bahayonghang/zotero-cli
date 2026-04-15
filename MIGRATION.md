# Python `zotero-cli-cc` to Rust `zot`

The Rust port keeps the `zot` binary name but redesigns the command surface around resource groups.

## Common command mapping

| Python-style command | Rust command |
| --- | --- |
| `zot search QUERY` | `zot library search QUERY` |
| `zot list` | `zot library list` |
| `zot read KEY` | `zot item get KEY` |
| `zot export KEY --format bibtex` | `zot item export KEY --format bibtex` |
| `zot cite KEY --style apa` | `zot item cite KEY --style apa` |
| `zot pdf KEY` | `zot item pdf KEY` |
| `zot open KEY` | `zot item open KEY` |
| `zot add --doi ...` | `zot item create --doi ...` |
| `zot update KEY --title ...` | `zot item update KEY --title ...` |
| `zot delete KEY` | `zot item trash KEY` |
| `zot trash restore KEY` | `zot item restore KEY` |
| `zot attach KEY --file file.pdf` | `zot item attach KEY --file file.pdf` |
| `zot note KEY --add ...` | `zot item note add KEY --content ...` |
| `zot tag KEY --add foo` | `zot item tag add KEY --tag foo` |
| `zot collection list` | `zot collection list` |
| `zot collection items COL` | `zot collection items COL` |
| `zot workspace query ...` | `zot workspace query ...` |
| `zot update-status --apply` | `zot sync update-status --apply` |

## JSON output

Rust commands with `--json` now return:

- success: `{"ok": true, "data": ..., "meta": ...}`
- failure: `{"ok": false, "error": {"code": "...", "message": "...", "hint": "..."}}`

## Notes

- The Rust port does not provide a compatibility shim for the legacy command names.
- `zot mcp serve` is reserved but not yet implemented.
