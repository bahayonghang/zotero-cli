"""Read-only review of the audit plan; writes only its own result file."""
import json
from pathlib import Path
import re
import subprocess

root=Path.cwd()
base=root/'.trellis/tasks'
parent=base/'09-28-evergreen-harness-review'
meta=json.loads((parent/'task.json').read_text(encoding='utf-8'))
names=[parent.name]+meta['children']
issues=[]
links=0
artifacts=0
for name in names:
    folder=base/name
    data=json.loads((folder/'task.json').read_text(encoding='utf-8'))
    if data['status']!='planning': issues.append(name+': not planning')
    if name!=parent.name and data['parent']!=parent.name: issues.append(name+': parent mismatch')
    if data.get('meta',{}).get('approval_status')!='awaiting-user': issues.append(name+': approval metadata')
    for file in ('prd.md','design.md','implement.md'):
        path=folder/file
        if not path.is_file(): issues.append(str(path)+': missing'); continue
        text=path.read_text(encoding='utf-8'); artifacts+=1
        if any(word in text for word in ('TBD','TODO','_example','调查中','待补充')): issues.append(str(path)+': placeholder')
    for file in ('implement.jsonl','check.jsonl'):
        path=folder/file
        rows=[json.loads(line) for line in path.read_text(encoding='utf-8').splitlines() if line.strip()]
        if not rows: issues.append(str(path)+': empty')
        for row in rows:
            if not (root/row['file']).is_file(): issues.append(row['file']+': context missing')
    for path in folder.rglob('*.md'):
        for target in re.findall(r'\]\(([^)]+)\)',path.read_text(encoding='utf-8')):
            if '://' in target or target.startswith('#'): continue
            links+=1
            if not (path.parent/target.split('#')[0]).exists(): issues.append(str(path)+': broken link '+target)
checks=json.loads((parent/'research/task-validation.json').read_text(encoding='utf-8'))
if len(checks)!=5 or any(r['exit_code']!=0 for r in checks): issues.append('task.py validate failed')
for stem,expected in [('just-ci',0),('docs-build',0),('actionlint',0),('cargo-audit',1),('cargo-deny',1)]:
    result=json.loads((parent/'research'/f'{stem}.result.json').read_text(encoding='utf-8-sig'))
    if result['exit_code']!=expected: issues.append(stem+': unexpected result')
anchors=[('Cargo.lock',1558,'name = "rustls"'),('Cargo.lock',1559,'0.23.38'),('README.md',15,'through Zotero Desktop'),('README.zh-CN.md',15,'Zotero Desktop'),('AGENTS.md',90,'no Trellis auto platform marker'),('CLAUDE.md',1,'@AGENTS.md'),('.trellis/scripts/common/workflow_phase.py',135,'def _platform_matches'),('.trellis/scripts/common/workflow_phase.py',145,'def resolve_effective_platform'),('src/zot-cli/src/commands/item/merge.rs',126,'WebMergeWriter')]
for path,line,token in anchors:
    text=(root/path).read_text(encoding='utf-8').splitlines()
    if token not in text[line-1]: issues.append(f'{path}:{line}: source anchor mismatch')
for args in (['git','diff','--quiet'],['git','diff','--cached','--quiet']):
    if subprocess.run(args).returncode: issues.append('pre-existing tracked files changed')
result={'status':'PASS' if not issues else 'FAIL','task_count':len(names),'planning_artifacts':artifacts,'context_manifests':len(names)*2,'source_anchors':len(anchors),'local_links_checked':links,'all_tasks_planning':True,'tracked_diff_unchanged':not any('tracked' in x for x in issues),'issues':issues,'review_scope':'Structural and source-anchor review by the main agent; not an independent reviewer or implementation validation.'}
(parent/'research/plan-review.json').write_text(json.dumps(result,ensure_ascii=False,indent=2),encoding='utf-8')
print(json.dumps(result,ensure_ascii=True,indent=2))
raise SystemExit(bool(issues))
