#!/usr/bin/env python3
"""Enforce transitive domain purity and production/test dependency boundaries."""
from pathlib import Path
import json
import subprocess

metadata = json.loads(subprocess.check_output(['cargo', 'metadata', '--locked', '--format-version', '1'], text=True))
packages = {p['id']: p for p in metadata['packages']}
nodes = {n['id']: n for n in metadata['resolve']['nodes']}
domain = next(p['id'] for p in packages.values() if p['name'] == 'cijian-domain')
seen = set()
def walk(identifier):
    if identifier in seen:
        return
    seen.add(identifier)
    p = packages[identifier]
    forbidden = ('gpui', 'rusqlite', 'sqlx', 'reqwest', 'tokio', 'rodio', 'rfd', 'keyring')
    assert not p['name'].startswith(forbidden), f'domain IO dependency: {p["name"]}'
    # Workspace feature unification also includes GPUI; inspect the domain's
    # declared chrono feature contract below rather than the global feature set.
    for dep in nodes[identifier]['deps']:
        if any(k['kind'] != 'dev' for k in dep['dep_kinds']):
            walk(dep['pkg'])
walk(domain)
chrono = next(d for d in packages[domain]['dependencies'] if d['name'] == 'chrono')
assert not chrono['uses_default_features'] and 'clock' not in chrono['features']
for source in Path('crates/cijian-domain/src').rglob('*.rs'):
    text = source.read_text()
    for forbidden in ['std::fs', 'std::net', 'SystemTime::now', 'Utc::now', 'Local::now', 'thread_rng', 'rand::rng', 'std::env', 'std::process']:
        assert forbidden not in text, f'{source}: forbidden domain IO: {forbidden}'
for node in nodes.values():
    if packages[node['id']]['name'].startswith('cijian-'):
        for dep in node['deps']:
            if packages[dep['pkg']]['name'] == 'cijian-testing':
                assert all(k['kind'] == 'dev' for k in dep['dep_kinds']), 'test doubles in production'
print(f'Passed: {len(seen)} transitive domain dependencies checked; production/test boundaries valid')
