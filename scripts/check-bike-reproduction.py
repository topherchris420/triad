"""Compare a clean Bike Sharing re-preparation with the committed frozen files.

Usage: python scripts/check-bike-reproduction.py NEW_PREPARED_DIRECTORY

Checks hourly rows, eligibility, every prediction, error scale and error
probability, provenance counts, protocol and model state (coefficients, scalers,
trees) with explicit floating-point tolerances, and verifies each run's own
model-file hashes. Byte hashes may legitimately differ across platforms.
"""
import hashlib
import json
import sys
from pathlib import Path

import numpy as np

BASE = Path('benchmarks/bike-sharing')
SKIP = {'model_sha256', 'preparation_source_sha256', 'python', 'feature_matrices_sha256', 'software', 'archive_sha256', 'archive_matches_recorded'}


def compare(a, b, path='root'):
    if isinstance(a, dict):
        assert isinstance(b, dict) and a.keys() == b.keys(), path
        for k in a:
            if k not in SKIP:
                compare(a[k], b[k], f'{path}.{k}')
    elif isinstance(a, list):
        assert isinstance(b, list) and len(a) == len(b), path
        if a and all(isinstance(v, (int, float)) and not isinstance(v, bool) for v in a):
            np.testing.assert_allclose(a, b, rtol=1e-8, atol=1e-9, err_msg=path)
        else:
            for i, (x, y) in enumerate(zip(a, b)):
                compare(x, y, f'{path}[{i}]')
    elif isinstance(a, float) or isinstance(b, float):
        np.testing.assert_allclose(a, b, rtol=1e-8, atol=1e-9, err_msg=path)
    else:
        assert a == b, (path, a, b)


def main():
    candidate = Path(sys.argv[1])
    for name in ('manifest.json', 'provenance.json', 'ridge-model.json', 'hgb-model.json'):
        compare(json.loads((BASE / name).read_bytes()), json.loads((candidate / name).read_bytes()), name)
    for root in (BASE, candidate):
        manifest = json.loads((root / 'manifest.json').read_bytes())
        protocol = BASE / 'protocol.json' if root == BASE else root / 'protocol.json'
        assert hashlib.sha256(protocol.read_bytes()).hexdigest() == manifest['protocol_sha256']
        for model in manifest['models']:
            digest = hashlib.sha256((root / f"{model['model_id']}-model.json").read_bytes()).hexdigest()
            assert digest == model['model_sha256'], (root, model['model_id'])
    print('PASS: clean re-preparation reproduces rows, eligibility, predictions, error scales, error probabilities and model states; model byte hashes verified for each run.')


if __name__ == '__main__':
    main()
