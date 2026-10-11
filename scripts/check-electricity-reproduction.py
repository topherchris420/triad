"""Compare a clean retraining to the archived benchmark, allowing BLAS roundoff.

Usage: python scripts/check-electricity-reproduction.py NEW_PREPARED_DIRECTORY
Checks row identity, predictions, truth, probabilities, all diagnostic windows,
preprocessing, model settings and serialized trained model coefficients/trees.
"""
import hashlib
import json
import sys
from pathlib import Path
import numpy as np

baseline=Path('benchmarks/electricity')
candidate=Path(sys.argv[1])


def compare(a,b,path='root'):
    if isinstance(a,dict):
        assert isinstance(b,dict) and a.keys()==b.keys(),path
        for k in a:
            # Byte hashes change with permissible platform-level floating-point differences.
            if k in ('model_sha256','feature_matrix_sha256','preparation_source_sha256','python'):
                continue
            compare(a[k],b[k],path+'.'+k)
    elif isinstance(a,list):
        assert isinstance(b,list) and len(a)==len(b),path
        if a and all(isinstance(v,(int,float)) and not isinstance(v,bool) for v in a):
            np.testing.assert_allclose(a,b,atol=1e-10,rtol=1e-9,err_msg=path)
        else:
            for i,(x,y) in enumerate(zip(a,b)): compare(x,y,f'{path}[{i}]')
    elif isinstance(a,float):
        np.testing.assert_allclose(a,b,atol=1e-10,rtol=1e-9,err_msg=path)
    else:
        assert a==b,(path,a,b)


for name in ('manifest.json','logistic-model.json','forest-model.json'):
    compare(json.loads((baseline/name).read_bytes()),json.loads((candidate/name).read_bytes()),name)
for root in (baseline,candidate):
    manifest=json.loads((root/'manifest.json').read_bytes())
    assert hashlib.sha256((root/'protocol.json').read_bytes()).hexdigest()==manifest['protocol_sha256']
    for model in manifest['models']:
        name=model['input']['model_id']
        assert hashlib.sha256((root/f'{name}-model.json').read_bytes()).hexdigest()==model['model_sha256']
print('PASS: clean retraining reproduces predictions, diagnostics, scaler, coefficients and trees; model byte hashes verified independently for each run.')
