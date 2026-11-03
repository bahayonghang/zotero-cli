"""Interactive acceptance against the explicitly isolated Zotero instance.

Run without -O. User handles all authorization dialogs; never retry a scenario
that may have committed. All mutations use cargo run -q -p zot-cli -- --json.
"""
import argparse
import json
import os
from pathlib import Path
import subprocess
import urllib.request

ROOT = next(p for p in Path(__file__).resolve().parents if (p / 'Cargo.toml').is_file())
RUNTIME = ROOT / 'target/local-organization-acceptance/2026-11-02'
FIXTURE = json.loads((RUNTIME / 'fixture.json').read_text(encoding='utf-8'))
assert Path(FIXTURE['data_dir']).resolve() == (RUNTIME / 'data').resolve()
assert FIXTURE.get('server_id') and FIXTURE['server_id'] != '2ZxitCcOtvOu'
OPENER = urllib.request.build_opener(urllib.request.ProxyHandler({}))
ENV = dict(os.environ, ZOT_DATA_DIR=str(RUNTIME / 'data'), ZOT_API_KEY='', ZOT_LIBRARY_ID='')
ITEMS = FIXTURE['items']
COLS = FIXTURE['collections']
EVENTS = []


def save(name, value):
    (RUNTIME / name).write_text(json.dumps(value, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')


def read(path):
    req = urllib.request.Request('http://127.0.0.1:23119/api/' + path,
                                 headers={'Zotero-Server-ID': FIXTURE['server_id']})
    with OPENER.open(req, timeout=10) as response:
        assert response.headers['Zotero-Server-ID'] == FIXTURE['server_id']
        return json.load(response)


def snapshot():
    items = [read('users/0/items/' + key) for key in ITEMS]
    assert all(item['data']['extra'] == 'Synthetic organization acceptance. Not a real publication.' for item in items)
    children = read('users/0/items/' + ITEMS[0] + '/children')
    assert {child['key'] for child in children} == set(FIXTURE['children'])
    return {'items': items, 'children': sorted(children, key=lambda child: child['key']),
            'collections': read('users/0/collections')}


def cli(args, error=None):
    result = subprocess.run(['cargo', 'run', '-q', '-p', 'zot-cli', '--', '--json', *args],
                            cwd=ROOT, env=ENV, capture_output=True, text=True, encoding='utf-8', timeout=180)
    if not result.stdout.strip():
        raise RuntimeError(f'CLI produced no JSON (exit {result.returncode}): {result.stderr}')
    output = json.loads(result.stdout)
    EVENTS.append({'args': args, 'exit_code': result.returncode, 'output': output})
    save(SCENARIO + '-events.json', EVENTS)
    if error:
        assert result.returncode == 1 and output['ok'] is False, output
        assert output['error']['code'] == error, output
        return output
    assert result.returncode == 0 and output['ok'] is True, output
    return output['data']


def confirm(args, preview):
    field = 'expected_library_version' if preview['operation'] == 'create-collection' else 'expected_version'
    return [*args, '--confirm', '--expected-server-id', preview['server_id'],
            '--' + field.replace('_', '-'), str(preview[field])]


def apply(args, deny=False):
    preview = cli(args)
    assert preview['state'] == 'preview' and not preview['write_accepted'], preview
    assert preview['server_id'] == FIXTURE['server_id']
    print(json.dumps({'scenario': SCENARIO, 'preview': preview}, ensure_ascii=False), flush=True)
    if deny:
        cli(confirm(args, preview), 'local-api-forbidden')
        return None
    result = cli(confirm(args, preview))
    assert result['state'] == 'applied' and result['verified'] and result['write_accepted'], result
    return result


def tags(action, *names):
    args = ['item', 'tag', action, ITEMS[0], '--write-backend', 'local']
    for name in names:
        args += ['--tag', name]
    return args


def create(name, parent=None):
    args = ['collection', 'create', name, '--write-backend', 'local']
    return args + (['--parent', parent] if parent else [])


def unchanged_fields(before, after):
    assert before['children'] == after['children'], 'Children changed'
    ignored = {'version', 'dateModified', 'tags', 'collections'}
    for old, new in zip(before['items'], after['items']):
        assert {k: v for k, v in old['data'].items() if k not in ignored} == {
            k: v for k, v in new['data'].items() if k not in ignored}, 'Unrelated item field changed'


parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('scenario', choices=['preview', 'deny', 'tag-add', 'tag-remove', 'create',
                                        'organize-plan', 'organize-apply', 'save-stale', 'reject-stale', 'restart'])
SCENARIO = parser.parse_args().scenario
before = snapshot()
evidence = {'scenario': SCENARIO, 'before': before, 'events': EVENTS, 'passed': False}
try:
    if SCENARIO == 'preview':
        assert cli(tags('add', '待读', 'needs review', 'C++/安全'))['state'] == 'preview'
        assert cli(tags('add', 'automatic-control'))['state'] == 'unchanged'
        assert cli(tags('remove', 'nonexistent'))['state'] == 'unchanged'
        wrong = cli(tags('add', '待读'))
        wrong['server_id'] = 'deliberately-wrong-instance'
        cli(confirm(tags('add', '待读'), wrong), 'local-api-conflict')
    elif SCENARIO == 'deny':
        apply(tags('add', '待读'), deny=True)
    elif SCENARIO == 'tag-add':
        apply(tags('add', '待读', 'needs review', 'C++/安全'))
        assert cli(tags('add', '待读', 'automatic-control'))['state'] == 'unchanged'
    elif SCENARIO == 'tag-remove':
        apply(tags('remove', '待读', 'needs review', 'C++/安全'))
        assert cli(tags('remove', '待读', 'nonexistent'))['state'] == 'unchanged'
    elif SCENARIO == 'create':
        root = apply(create('验收根分类'))['collection_key']
        child = apply(create('验收子分类', root))['collection_key']
        cli(create('验收根分类'), 'local-collection-duplicate')
        other = apply(create('验收根分类', COLS['Control']))['collection_key']
        save('created.json', {'root': root, 'child': child, 'same_name_other_parent': other})
        member = ['collection', 'add-item', child, ITEMS[2], '--write-backend', 'local']
        apply(member)
        assert cli(member)['state'] == 'unchanged'
    elif SCENARIO == 'organize-plan':
        created = json.loads((RUNTIME / 'created.json').read_text(encoding='utf-8'))
        request = {'schema_version': 1, 'items': [
            {'item_key': ITEMS[0], 'add_tags': ['待读'], 'add_collections': [created['root']], 'reason': 'Synthetic first decision'},
            {'item_key': ITEMS[1], 'remove_tags': ['obsolete'], 'move_collection': {'from': COLS['Source'], 'to': created['child']}, 'reason': 'Synthetic second decision'},
            {'item_key': ITEMS[2], 'add_tags': ['已读'], 'remove_collections': [COLS['Source']], 'reason': 'Synthetic third decision'},
        ]}
        save('requests.json', request)
        summary = cli(['library', 'organize', 'plan', '--file', str(RUNTIME / 'requests.json'),
                       '--output', str(RUNTIME / 'plan.json'), '--write-backend', 'local'])
        assert summary['changed_count'] == 3 and summary['item_count'] == 3
        save('plan-summary.json', summary)
        print(json.dumps(summary, ensure_ascii=False), flush=True)
    elif SCENARIO == 'organize-apply':
        summary = json.loads((RUNTIME / 'plan-summary.json').read_text(encoding='utf-8'))
        report = cli(['library', 'organize', 'apply', '--plan', str(RUNTIME / 'plan.json'),
                      '--confirm', '--expected-plan-sha256', summary['plan_sha256'],
                      '--report', str(RUNTIME / 'report.json'), '--write-backend', 'local'])
        assert report['state'] == 'applied' and report['applied_count'] == 3, report
        assert all(item['verified'] for item in report['items'])
        assert json.loads((RUNTIME / 'report.json').read_text(encoding='utf-8')) == report
        save('expected-final.json', snapshot())
    elif SCENARIO == 'save-stale':
        args = tags('add', 'must-not-apply-stale')
        save('stale.json', {'args': args, 'preview': cli(args)})
        args = create('Must not create stale')
        save('stale-create.json', {'args': args, 'preview': cli(args)})
    elif SCENARIO == 'reject-stale':
        for name in ['stale.json', 'stale-create.json']:
            saved = json.loads((RUNTIME / name).read_text(encoding='utf-8'))
            cli(confirm(saved['args'], saved['preview']), 'local-api-conflict')
        save('expected-final.json', snapshot())
    elif SCENARIO == 'restart':
        expected = json.loads((RUNTIME / 'expected-final.json').read_text(encoding='utf-8'))
        assert before == expected, 'Restart changed API state'
        assert cli(tags('add', 'automatic-control'))['state'] == 'unchanged'
    after = snapshot()
    unchanged_fields(before, after)
    if SCENARIO in ['preview', 'deny', 'organize-plan', 'save-stale', 'reject-stale', 'restart']:
        assert before == after, 'Read-only/rejected operation changed state'
    for item in after['items']:
        automatic = [tag for tag in item['data']['tags'] if tag['tag'] == 'automatic-control']
        assert automatic == [{'tag': 'automatic-control', 'type': 1}], automatic
        assert COLS['Control'] in item['data']['collections']
    evidence['after'] = after
    evidence['passed'] = True
finally:
    save(SCENARIO + '.json', evidence)
print(json.dumps({'scenario': SCENARIO, 'passed': True, 'events': len(EVENTS)}), flush=True)
