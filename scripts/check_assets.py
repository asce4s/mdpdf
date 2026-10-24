#!/usr/bin/env python3
"""Verify checked-in vendor bytes and local font references without network."""
import hashlib
import json
from pathlib import Path
import re

assets = Path(__file__).resolve().parents[1] / 'assets'
count = 0
for package in json.loads((assets / 'vendor-manifest.json').read_text()):
    for name, expected in package['files'].items():
        path = assets / name
        assert hashlib.sha256(path.read_bytes()).hexdigest() == expected, path
        if path.suffix == '.woff2':
            assert path.read_bytes()[:4] == b'wOF2', path
        count += 1
for theme in json.loads((assets / 'syntax/manifest.json').read_text()):
    for name, source in theme['files'].items():
        path = assets / 'syntax' / name
        assert hashlib.sha256(path.read_bytes()).hexdigest() == source['sha256'], path
        count += 1
for name in re.findall(r'url\(/assets/([^\)]+)\)', (assets / 'fonts/fonts.css').read_text()):
    assert (assets / name).is_file(), name
print(f'Verified {count} pinned vendor files and every font CSS reference.')
for folder in ['vendor/katex', 'vendor/codemirror']:
    manifest = json.loads((assets / folder / 'manifest.json').read_text())
    entries = manifest.get('files', {'editor.js': manifest.get('sha256')})
    for name, expected in entries.items():
        path = assets / folder / name
        assert hashlib.sha256(path.read_bytes()).hexdigest() == expected, path
    print(f'Verified {folder}: {len(entries)} pinned files.')
