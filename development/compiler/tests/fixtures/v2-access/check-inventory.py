#!/usr/bin/env python3
"""Validate fixture inventory/range integrity only; never parse or compile Nocter."""
from collections import Counter
import json
from pathlib import Path


def require(condition, message):
    if not condition:
        raise SystemExit(message)


root = Path(__file__).resolve().parent
manifest = json.loads((root / 'manifest.json').read_text())
require(manifest['schema'] == 1 and manifest['parse_goal'] == 'V2CoreUnit', 'unknown fixture schema/goal')
require(manifest['compiler_status'] == 'declarations-only', 'update validation when body checking begins')
seen = set()
files = set()
shapes = set()
counts = Counter()
for case in manifest['cases']:
    name = case['file']
    require(case['id'] not in seen, 'duplicate fixture identity')
    require(name == case['id'] + '.nct' and Path(name).name == name, 'nonlocal/incorrect fixture path')
    require(name not in files, 'duplicate source input')
    seen.add(case['id'])
    files.add(name)
    source = (root / name).read_text()
    require(source.endswith('\n') and '\r' not in source, 'source must have LF and a final newline')
    require(case['expectation'] in ('accept', 'check-reject', 'syntax-reject', 'declaration-reject'), 'unknown expected stage')
    require(bool(case['reason']), 'missing distinguishing rule')
    if case['expectation'] == 'accept':
        require('primary_text' not in case, 'accepted input has a failure projection')
    else:
        require(source.count(case['primary_text']) == 1, 'failure projection must identify one source range')
    if case['expectation'] != 'syntax-reject':
        shape = Path(name).with_suffix('.shape').name
        require((root / shape).read_text().startswith('V2CoreUnit\n'), 'missing/wrong-goal syntax shape')
        shapes.add(shape)
    if case['expectation'] == 'declaration-reject':
        require(case.get('code') in ('E0240', 'E0241', 'E0341'), 'missing header diagnostic code')
        require(isinstance(case.get('diagnostic_count', 1), int) and case.get('diagnostic_count', 1) > 0, 'invalid diagnostic count')
        if 'related_text' in case:
            require(source.count(case['related_text']) == 1, 'related projection must identify one source range')
    counts[case['expectation']] += 1
require(files == {path.name for path in root.glob('*.nct')}, 'unregistered or missing source fixture')
require(shapes == {path.name for path in root.glob('*.shape')}, 'unregistered syntax shape')
print(json.dumps({'syntax_shapes': len(shapes), 'fixture_inventory': 'valid', 'cases': len(seen), 'expected': dict(sorted(counts.items())),
                  'compiled_fixture_cases': 0}, sort_keys=True))
