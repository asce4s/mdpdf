#!/usr/bin/env python3
"""Vendor immutable MIT syntax sources and convert Nord to TextMate format."""
from pathlib import Path
from urllib.request import urlopen
from urllib.parse import quote
import hashlib
import json
import plistlib
import re

ROOT = Path(__file__).resolve().parents[1] / 'assets/syntax'
SOURCES = [
 ('github-light', 'Colorsublime/Colorsublime-Themes', '56b1b09c4d63c61a9fbfdd4eeb517c023497b64c', 'themes/GitHub_Light.tmTheme', 'LICENSE'),
 ('one-dark', 'andresmichel/one-dark-theme', '9ecab1531c983680897b9a77190262561ce4fe0e', 'One Dark.tmTheme', 'LICENSE'),
 ('solarized-light', 'altercation/solarized', '62f656a02f93c5190a8753159e34b385588d5ff3', 'textmate-colors-solarized/Solarized (light).tmTheme', 'LICENSE'),
 ('solarized-dark', 'altercation/solarized', '62f656a02f93c5190a8753159e34b385588d5ff3', 'textmate-colors-solarized/Solarized (dark).tmTheme', 'LICENSE'),
 ('nord', 'arcticicestudio/nord-sublime-text', '91eae63dc83ed501aa133d8f3266c301ab0cbf68', 'Nord.sublime-color-scheme', 'license'),
]
manifest = []
for name, repo, revision, source, license_path in SOURCES:
    files = {}
    for source_path, destination in [(source, name + ('.sublime-color-scheme' if name == 'nord' else '.tmTheme')), (license_path, name + '.LICENSE')]:
        url = f'https://raw.githubusercontent.com/{repo}/{revision}/{quote(source_path)}'
        data = urlopen(url, timeout=30).read()
        (ROOT / destination).write_bytes(data)
        files[destination] = {'url': url, 'sha256': hashlib.sha256(data).hexdigest()}
    manifest.append({'name': name, 'repository': repo, 'revision': revision, 'files': files})
    if name == 'nord':
        text = (ROOT / 'nord.sublime-color-scheme').read_text()
        text = re.sub(r'/\*.*?\*/', '', text, flags=re.S)
        doc = json.loads(text)
        def color(value):
            while value.startswith('var('):
                value = doc['variables'][value[4:-1]]
            value = {'lime': '#00ff00', 'black': '#000000', 'white': '#ffffff'}.get(value, value)
            if not re.fullmatch(r'#[0-9a-fA-F]{6,8}', value):
                raise ValueError('Unsupported token colour: ' + value)
            return value
        settings = [{'settings': {key: color(doc['globals'][key]) for key in ['foreground','background']}}]
        for rule in doc['rules']:
            entry = {'name': rule.get('name', ''), 'scope': rule['scope'], 'settings': {}}
            for key in ['foreground', 'background']:
                if key in rule:
                    entry['settings'][key] = color(rule[key])
            if 'font_style' in rule:
                entry['settings']['fontStyle'] = rule['font_style']
            settings.append(entry)
        (ROOT / 'nord.tmTheme').write_bytes(plistlib.dumps({'name': 'Nord', 'settings': settings}))
    print('Vendored', name, flush=True)
(ROOT / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
