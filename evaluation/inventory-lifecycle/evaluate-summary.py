#!/usr/bin/env python3
"""Bounded authored-summary navigation fixture, not an LLM quality benchmark."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile


def tokens(text):
    ascii_count = sum(ord(c) < 128 for c in text)
    return (ascii_count + 3) // 4 + sum(ord(c) >= 128 for c in text)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    opts = parser.parse_args()
    binary = opts.binary.resolve()
    with tempfile.TemporaryDirectory(prefix='belay-summary-v1-', dir=os.environ.get('TMPDIR')) as temporary:
        root = Path(temporary)
        def call(args):
            p = subprocess.run([str(binary), *args], cwd=root, capture_output=True, text=True)
            assert p.returncode == 0, (args, p.returncode, p.stderr)
            return p.stdout
        call(['init'])
        body = '''## Outcome
The isolated staging fixture accepts valid UTF-8 CSV amounts and rejects malformed amounts. Only the fixture was evaluated; a completed Work status does not verify a Goal.

## Local rationale
Strict amount parsing was chosen within this fixture because silently truncating invalid amounts would obscure input errors. This rationale is not a production adoption decision.

## Constraints
Production migration is excluded. The fixture does not authorize changes to a production system.

## Unresolved
Legacy Shift-JIS inputs remain untested. No claim about every encoding or importer is supported.

## Lesson
Hypothesis: explicit rejection might reduce silent corruption elsewhere. No general result is established.

## Artifacts
staging-import.rs is an illustrative fixture artifact reference. External logs are references only and are not preserved automatically.
'''
        work = re.search(r'WRK-[A-Za-z0-9-]+', call(['add', 'work', '--title', 'authored-summary-fixture', '--body', body]))[0]
        call(['status', work, 'completed'])
        record = re.search(r'EVD-[A-Za-z0-9-]+', call(['verify', 'record', '--kind', 'test', '--verdict', 'pass', '--source', 'synthetic-authored-fixture', '--summary', 'Fixed local fixture only; not production validation', '--verifies', work]))[0]
        query = ['context', 'compile', work, '--budget', '5000', '--format', 'agent']
        before = [call(query), call(['show', work])]
        sections = [
            ('Outcome', 'Valid UTF-8 staging fixture amounts accepted; malformed amounts rejected. No Goal verification.', [work, record]),
            ('Rationale and scope', 'Strict parsing avoids silently truncated amounts within this fixture; not production adoption.', [work]),
            ('Constraints', 'Production migration is excluded.', [work]),
            ('Unresolved', 'Legacy Shift-JIS inputs remain untested.', [work]),
            ('Hypothesis', 'Explicit rejection might reduce silent corruption elsewhere; no general result established.', [work]),
            ('Artifacts', 'staging-import.rs is illustrative. External logs remain references only.', [work]),
        ]
        artifact = {'schema_version': 1, 'artifact_id': 'navigation-fixture', 'artifact_revision': 1, 'generator_version': 'authored-semantic-fixture-v1', 'kind': 'derived-summary', 'authority': 'derived-only', 'sources': json.loads(call(['lifecycle', 'summary', 'sources', work, record])), 'sections': [{'heading': h, 'text': t, 'source_ids': ids} for h,t,ids in sections], 'limitations': ['Bounded authored fixture; no measured model generalization.']}
        path = root / 'summary.json'; path.write_text(json.dumps(artifact))
        call(['lifecycle', 'summary', 'store', '--file', str(path)])
        after = [call(query)]
        for sentinel in ('Production migration is excluded', 'Shift-JIS inputs remain untested', 'not production adoption', 'no general result', 'staging-import.rs', record, work):
            assert sentinel in after[0], sentinel
        assert 'not verification' in after[0]
        report = {'schema_version': 1, 'binary_sha256': hashlib.sha256(binary.read_bytes()).hexdigest(), 'scope': 'Scripted same-compiler same-original-data authored fixture; prose fidelity reviewed against six source sections, not string equivalence as quality certification.', 'before': {'reads': len(before), 'tokens': sum(map(tokens, before)), 'outputs': before}, 'after': {'reads': len(after), 'tokens': sum(map(tokens, after)), 'outputs': after}, 'artifact': artifact, 'source_body': body}
        opts.output.write_text(json.dumps(report, indent=2) + '\n')
        print(json.dumps({k: {f: v for f,v in report[k].items() if f != 'outputs'} for k in ('before', 'after')}))


if __name__ == '__main__':
    main()
