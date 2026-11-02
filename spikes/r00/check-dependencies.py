#!/usr/bin/env python3
"""Validate the real locked consumer graph, not an adjacent upstream checkout."""
import json
from pathlib import Path
import subprocess
import tomllib

root = Path(__file__).resolve().parent
lock = tomllib.loads((root / 'Cargo.lock').read_text())
revision = '2e13169fceb4ac328e1361aa143632025598872f'
kit_revision = 'bb85882e1faab845d9e7be33b69a152879a90f69'
for name in ['gpui-kit', 'gpui-base', 'gpui-pre', 'gpui-fast']:
    packages = [p for p in lock['package'] if p['name'] == name]
    assert len(packages) == 1, (name, packages)
    assert packages[0]['source'].endswith('#' + revision), packages[0]
for package in lock['package']:
    source = package.get('source', '')
    if source.startswith('git+https://github.com/longbridge/gpui-kit'):
        assert source.endswith('#' + kit_revision), package
manifest = tomllib.loads((root / 'Cargo.toml').read_text())
patches = [item for group in manifest['patch'].values() for item in group.values()]
assert len(patches) == 9
assert all(item.get('rev') == revision and 'path' not in item for item in patches)
metadata = json.loads(subprocess.check_output([
    'cargo', '+1.99.1', 'metadata', '--manifest-path', str(root / 'Cargo.toml'),
    '--locked', '--format-version', '1', '--no-deps',
], text=True))
assert all(dependency.get('path') is None for p in metadata['packages'] for dependency in p['dependencies'])
print(json.dumps({'uniqueFacades': True, 'rootPatches': len(patches),
                  'materialRevision': revision, 'kitRevision': kit_revision,
                  'externalPathDependencies': False}, indent=2))
