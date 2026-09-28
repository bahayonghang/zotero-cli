"""Read-only audit probe. Does not change Trellis configuration."""
import json
from pathlib import Path
import subprocess
import sys

root = Path.cwd()
out = root / '.trellis/tasks/09-28-evergreen-harness-review/research'
sys.path.insert(0, str(root / '.trellis/scripts'))
from common.workflow_phase import resolve_effective_platform, _platform_matches

pairs = [('claude', 'Claude Code'), ('codex', 'codex-sub-agent'), ('grok', 'Grok'), ('kimi', 'Kimi Code'), ('omp', 'Oh My Pi')]
rows = []
for platform, label in pairs:
    effective = resolve_effective_platform(platform, {})
    p = subprocess.run([sys.executable, '.trellis/scripts/get_context.py', '--mode', 'phase', '--step', '2.1', '--platform', platform], capture_output=True, text=True, encoding='utf-8', errors='replace')
    (out / f'phase-2.1-{platform}.txt').write_text(p.stdout + p.stderr, encoding='utf-8')
    rows.append({'platform': platform, 'effective': effective, 'workflow_label': label, 'label_matches': _platform_matches(effective, [label]), 'exit_code': p.returncode, 'rendered_chars': len(p.stdout), 'has_implement_agent': 'trellis-implement' in p.stdout, 'has_no_direct_code_rule': 'Do NOT write code directly' in p.stdout})
(out / 'platform-probe.json').write_text(json.dumps(rows, ensure_ascii=False, indent=2), encoding='utf-8')
print(json.dumps(rows, ensure_ascii=False, indent=2))
