#!/usr/bin/env python3
"""Isolated fixed lifecycle matrix; never mutates the source checkout."""
import argparse
from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tarfile
import tempfile
import time

BASE = '9d4b16e341a23ee505fad1d2315fb389b892a298'
STAMP = '2026-09-30T12:00:00Z'


def call(binary, root, args, codes=(0,)):
    start = time.perf_counter()
    p = subprocess.run([str(binary), *args], cwd=root, capture_output=True, text=True)
    if p.returncode not in codes:
        raise RuntimeError(f'{args}: {p.returncode}: {p.stderr}\n{p.stdout}')
    return p.stdout, time.perf_counter() - start, p.returncode


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def originals(root):
    belay = root / '.belay'
    paths = list((belay / 'entries').rglob('*.md')) + list((belay / 'evidence').rglob('*.ndjson')) + list((belay / 'evidence').rglob('*.json'))
    return {str(p.relative_to(belay)): digest(p) for p in paths if p.is_file()}


def footprint(root):
    paths = [p for p in (root / '.belay').rglob('*') if p.is_file() and 'state' not in p.relative_to(root / '.belay').parts]
    return {'tracked_candidate_files': len(paths), 'tracked_candidate_bytes': sum(p.stat().st_size for p in paths), 'loose_original_files': len(originals(root))}


def git_snapshot(root, message, initialize=False):
    if initialize:
        subprocess.run(['git', 'init', '-q'], cwd=root, check=True)
        subprocess.run(['git', 'config', 'user.name', 'Lifecycle fixture'], cwd=root, check=True)
        subprocess.run(['git', 'config', 'user.email', 'fixture@example.invalid'], cwd=root, check=True)
    subprocess.run(['git', 'add', '.belay'], cwd=root, check=True)
    subprocess.run(['git', '-c', 'core.hooksPath=/dev/null', 'commit', '-qm', message], cwd=root, check=True)
    paths = [p for p in (root / '.git/objects').rglob('*') if p.is_file()]
    return {'object_files': len(paths), 'object_bytes': sum(p.stat().st_size for p in paths), 'note': 'Loose Git object storage of isolated snapshot commits; no history rewrite or GC.'}


def record(binary, root, goal, number):
    out, elapsed, _ = call(binary, root, ['verify', 'record', '--kind', 'test', '--verdict', 'pass', '--source', 'fixed-concurrency-fixture', '--summary', f'fixture {number}', '--commit', 'unknown', '--captured-at', STAMP, '--verifies', goal])
    return {'id': re.search(r'EVD-[A-Za-z0-9-]+', out)[0], 'seconds': elapsed}


def new_repo(binary, root):
    root.mkdir()
    call(binary, root, ['init'])
    out, _, _ = call(binary, root, ['add', 'goal', '--title', 'fixed-lifecycle-fixture'])
    return re.search(r'GOAL-[A-Za-z0-9-]+', out)[0]


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--binary', type=Path, required=True)
    p.add_argument('--old', type=Path, required=True)
    p.add_argument('--output', type=Path, required=True)
    options = p.parse_args()
    binary, old = options.binary.resolve(), options.old.resolve()
    source = Path(__file__).resolve().parents[2]
    report = {'schema_version': 1, 'base_commit': BASE, 'binary_sha256': digest(binary), 'parallel': [], 'independent_copies': {}, 'real_snapshot': {}}
    with tempfile.TemporaryDirectory(prefix='belay-lifecycle-v1-', dir=os.environ.get('TMPDIR')) as temporary:
        parent = Path(temporary)
        for workers in (1, 8):
            for repeat in range(3):
                root = parent / f'parallel-{workers}-{repeat}'
                goal = new_repo(binary, root)
                with ThreadPoolExecutor(max_workers=workers) as pool:
                    records = list(pool.map(lambda n: record(binary, root, goal, n), range(workers * 25)))
                assert len({r['id'] for r in records}) == workers * 25
                call(binary, root, ['rebuild'])
                inventory = json.loads(call(binary, root, ['inventory', '--format', 'json'])[0])
                assert len(inventory['evidence']) == workers * 25
                report['parallel'].append({'writers': workers, 'repeat': repeat, 'records': records, 'reconstructed_count': len(inventory['evidence'])})
        one = parent / 'independent-one'
        goal = new_repo(binary, one)
        two = parent / 'independent-two'
        shutil.copytree(one, two)
        with ThreadPoolExecutor(max_workers=2) as pool:
            copies = list(pool.map(lambda root: [record(binary, root, goal, n) for n in range(25)], (one, two)))
        assert len({r['id'] for rows in copies for r in rows}) == 50
        for path in (two / '.belay/evidence/records').glob('*.json'):
            destination = one / '.belay/evidence/records' / path.name
            assert not destination.exists()
            shutil.copy2(path, destination)
        call(binary, one, ['rebuild'])
        merged = json.loads(call(binary, one, ['inventory', '--format', 'json'])[0])
        assert len(merged['evidence']) == 50
        before_legacy_write = originals(one)
        _, _, old_code = call(old, one, ['verify', 'record', '--kind', 'test', '--verdict', 'pass', '--source', 'old-cli-refusal', '--summary', 'must refuse', '--verifies', goal], codes=tuple(range(1, 256)))
        assert before_legacy_write == originals(one), 'legacy refusal modified originals'
        report['independent_copies'] = {'records': copies, 'merged_count': 50, 'old_cli_refusal_exit': old_code}
        # Archive only tracked source data at the frozen commit, not live user data.
        root = parent / 'real-snapshot'
        root.mkdir()
        archive = parent / 'baseline.tar'
        subprocess.run(['git', 'archive', '--format=tar', '-o', str(archive), BASE, '.belay'], cwd=source, check=True)
        with tarfile.open(archive) as tar:
            tar.extractall(root, filter='data')
        call(binary, root, ['init', '--reset-state'])
        git_before = git_snapshot(root, 'frozen raw corpus', initialize=True)
        before = originals(root)
        before_footprint = footprint(root)
        before_inventory = json.loads(call(binary, root, ['inventory', '--format', 'json', '--now', STAMP])[0])
        preview = call(binary, root, ['lifecycle', 'pack', 'preview'])[0]
        preview_path = root / 'preview.json'
        preview_path.write_text(preview)
        outcome = json.loads(call(binary, root, ['lifecycle', 'pack', 'apply', '--file', str(preview_path)])[0])
        remaining = json.loads(call(binary, root, ['lifecycle', 'pack', 'preview'])[0])
        assert not remaining['records'], 'eligible loose records remained after all-selected pack'
        packed_footprint = footprint(root)
        git_packed = git_snapshot(root, 'verified pack retains original payloads')
        _, rebuild_seconds, _ = call(binary, root, ['rebuild'])
        after_inventory = json.loads(call(binary, root, ['inventory', '--format', 'json', '--now', STAMP])[0])
        assert {r['source']['id'] for r in before_inventory['entries']} == {r['source']['id'] for r in after_inventory['entries']}
        assert {r['source']['id'] for r in before_inventory['evidence']} == {r['source']['id'] for r in after_inventory['evidence']}
        for entry in after_inventory['entries']:
            call(binary, root, ['show', entry['source']['id']])
        for evidence in after_inventory['evidence']:
            call(binary, root, ['show', evidence['source']['id']])
        call(binary, root, ['coverage', '--format', 'json'])
        call(binary, root, ['export', 'json', '--output', 'export.json'])
        call(binary, root, ['doctor'], codes=(0, 5))
        call(binary, root, ['lifecycle', 'pack', 'recover', outcome['receipt']])
        call(binary, root, ['lifecycle', 'pack', 'restore', outcome['pack_hash']])
        call(binary, root, ['sync'])
        after = originals(root)
        assert all(after.get(path) == sha for path, sha in before.items())
        report['real_snapshot'] = {'before': before_footprint, 'packed': packed_footprint, 'git_before': git_before, 'git_packed': git_packed, 'after_restore': footprint(root), 'pack': outcome, 'rebuild_seconds': rebuild_seconds, 'source_hashes_before': before, 'source_hashes_after': after, 'restored_exact_sources': len(before), 'remaining_eligible_loose_records': len(remaining['records']), 'entries': len(after_inventory['entries']), 'evidence': len(after_inventory['evidence'])}
    options.output.write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({k: v for k, v in report.items() if k not in ('parallel', 'real_snapshot', 'independent_copies')}, indent=2))


if __name__ == '__main__':
    main()
