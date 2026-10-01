#!/usr/bin/env python3
"""Exercise a single reviewed preview only on an isolated current-data copy."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import sqlite3
import subprocess
import tempfile

source = Path(__file__).resolve().parents[2]
binary = source / 'target/release/belay'
preview_file = Path(__file__).with_name('archive-proposal.json')
preview = json.loads(preview_file.read_text())
id = preview['source']['id']
with tempfile.TemporaryDirectory(prefix='belay-archive-preflight-', dir=os.environ.get('TMPDIR')) as temporary:
    root = Path(temporary)
    shutil.copytree(source / '.belay', root / '.belay')
    with sqlite3.connect(f'file:{source}/.belay/state/belay.sqlite?mode=ro', uri=True) as origin:
        with sqlite3.connect(root / '.belay/state/belay.sqlite') as destination:
            origin.backup(destination)
    def call(args, allowed=(0,)):
        p = subprocess.run([str(binary), *args], cwd=root, capture_output=True, text=True)
        assert p.returncode in allowed, (args, p.returncode, p.stderr)
        return {'exit': p.returncode, 'stdout': p.stdout, 'stderr': p.stderr}
    def diagnostics():
        return {name: call(args, allowed) for name, args, allowed in [
            ('doctor', ['doctor'], (0, 5)),
            ('coverage', ['coverage', '--format', 'json'], (0,)),
            ('export', ['export', 'json', '--output', 'export.json'], (0,)),
        ]}
    before = diagnostics()
    before_show = call(['show', id])
    receipt = call(['inventory', 'apply', '--file', str(preview_file)])
    after_show = call(['show', id])
    assert 'Status: archived' in after_show['stdout']
    call(['rebuild'])
    after = diagnostics()
    receipt_file = root / 'receipt.json'
    receipt_file.write_text(receipt['stdout'])
    restore = call(['inventory', 'restore', '--file', str(receipt_file)])
    restore_file = root / 'restore.json'
    restore_file.write_text(restore['stdout'])
    call(['inventory', 'apply', '--file', str(restore_file)])
    restored_show = call(['show', id])
    assert 'Status: completed' in restored_show['stdout']
    assert before['doctor']['exit'] == after['doctor']['exit']
    assert before['coverage']['stdout'] == after['coverage']['stdout']
    result = {'binary_sha256': hashlib.sha256(binary.read_bytes()).hexdigest(), 'source_preview': preview, 'scope': 'isolated copy only; actual source not changed', 'before': before, 'after': after, 'before_show': before_show, 'after_show': after_show, 'restored_show': restored_show, 'receipt': json.loads(receipt['stdout']), 'restore_preview': json.loads(restore['stdout'])}
    preview_file.with_name('archive-proposal-preflight.json').write_text(json.dumps(result, indent=2, ensure_ascii=False) + '\n')
    print('isolated archive/apply/rebuild/restore passed; original data unchanged')
