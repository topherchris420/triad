"""Retrieve pinned OpenML 151 and fit frozen models. Never imports benchmark results.

Run from repository root; --cache is a local directory, --output must be new.
No raw dataset is redistributed. All output JSON uses canonical LF bytes.
"""
import argparse
import hashlib
import importlib.metadata
import json
import platform
import urllib.request
from pathlib import Path

import numpy as np
from scipy.io import arff
from sklearn.ensemble import RandomForestClassifier
from sklearn.linear_model import LogisticRegression
from sklearn.preprocessing import StandardScaler


def digest(data):
    return hashlib.sha256(data).hexdigest()


def save(path, value):
    with path.open('xb') as f:
        f.write((json.dumps(value, separators=(',', ':'), allow_nan=False) + '\n').encode())


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--cache', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    protocol_bytes = Path('benchmarks/electricity/protocol.json').read_bytes()
    protocol = json.loads(protocol_bytes)
    args.cache.mkdir(parents=True, exist_ok=True)
    for name, url in {
        'metadata.json': 'https://www.openml.org/api/v1/json/data/151',
        'features.json': 'https://www.openml.org/api/v1/json/data/features/151',
        'electricity.arff': 'https://openml.org/data/v1/download/2419/electricity.arff',
    }.items():
        path = args.cache / name
        if not path.exists():
            with urllib.request.urlopen(url, timeout=120) as response:
                path.write_bytes(response.read())
    raw = (args.cache / 'electricity.arff').read_bytes()
    assert digest(raw) == protocol['dataset_sha256'], 'Dataset changed; do not silently substitute'
    meta = json.loads((args.cache / 'metadata.json').read_bytes())['data_set_description']
    assert meta['id'] == '151' and meta['version'] == '1' and meta['licence'] == 'Public'
    assert hashlib.md5(raw).hexdigest() == meta['md5_checksum']
    data, schema = arff.loadarff(args.cache / 'electricity.arff')
    expected = ['date', 'day', 'period', 'nswprice', 'nswdemand', 'vicprice', 'vicdemand', 'transfer', 'class']
    assert schema.names() == expected and len(data) == 45312
    a = np.column_stack([data[n].astype(float) for n in expected[:-1]])
    assert np.isfinite(a).all()
    idx = np.arange(len(data))
    day = idx // 48
    assert np.array_equal(np.rint(a[:, 2] * 47).astype(int), idx % 48)
    assert np.array_equal(a[:, 1].astype(int), (day + 1) % 7 + 1)
    # No label participates in ordering, features, scaling or alert diagnostics.
    x = np.column_stack([np.sin(2*np.pi*(idx % 48)/48), np.cos(2*np.pi*(idx % 48)/48),
                         np.sin(2*np.pi*a[:, 1]/7), np.cos(2*np.pi*a[:, 1]/7),
                         np.roll(a[:, 3:8], 48, axis=0)])
    splits = {s: (day >= lo) & (day < hi) for s, (lo, hi) in protocol['split_day_indices_half_open'].items()}
    assert all(not np.any(v & w) for i, v in enumerate(splits.values()) for w in list(splits.values())[i+1:])
    assert not any(v[:48].any() for v in splits.values())
    scaler = StandardScaler().fit(x[splits['training']])
    z = scaler.transform(x)
    assert set(data['class']) == {b'UP', b'DOWN'}
    # Training labels only are consumed by fits; all hyperparameters are in committed protocol.
    ytrain = (data['class'][splits['training']] == b'UP').astype(int)
    models = {'logistic': LogisticRegression(**protocol['models']['logistic']),
              'forest': RandomForestClassifier(**protocol['models']['forest'])}
    args.output.mkdir(parents=True, exist_ok=False)
    (args.output / 'protocol.json').write_bytes(protocol_bytes)
    for name in ('metadata.json', 'features.json'):
        (args.output / name).write_bytes((args.cache / name).read_bytes())
    provenance = {
        'source_url': meta['url'], 'openml_id': 151, 'version': 1, 'sha256': digest(raw),
        'md5': hashlib.md5(raw).hexdigest(), 'license_as_reported': meta['licence'],
        'license_note': 'Public is the OpenML metadata label, not a specific legal license. Raw data not redistributed; downstream users must review rights.',
        'rows': len(data), 'feature_schema': expected, 'missing_released_values': 0,
        'date_backward_jumps': (np.flatnonzero(np.diff(a[:, 0]) < 0) + 1).tolist(),
        'unique_normalized_dates': len(np.unique(a[:, 0])), 'day_blocks': 944,
        'chronology': 'conditional_published_row_order; absolute calendar timestamps unresolved',
        'upstream_preprocessing': 'ARFF relation declares Normalize and ReplaceMissingValues; cannot reverse global preprocessing or recover original missingness',
        'software': {n: importlib.metadata.version(n) for n in ('numpy', 'scipy', 'scikit-learn', 'joblib', 'threadpoolctl')},
        'python': platform.python_version(), 'split_counts': {s:int(v.sum()) for s,v in splits.items()},
        'scaler_mean': scaler.mean_.tolist(), 'scaler_scale': scaler.scale_.tolist(),
        'feature_matrix_sha256': digest(np.asarray(z[48:], dtype='<f8').tobytes()),
        'preparation_source_sha256': digest(Path(__file__).read_bytes()),
        'protocol_sha256': digest(protocol_bytes), 'features': protocol['features'],
    }
    manifest_models = []
    for name, model in models.items():
        model.fit(z[splits['training']], ytrain)
        p = model.predict_proba(z)[:, 1]
        score = np.minimum(p, 1-p)
        pred = (p > 0.5).astype(int)  # sklearn argmax resolves exact tie to DOWN
        dev = splits['development']
        reference = z[dev].mean(axis=0)
        reference_score = score[dev].mean()

        def windows(split):
            lo, hi = protocol['split_day_indices_half_open'][split]
            result = []
            for start in range(lo, hi, 7):
                end = min(start+7, hi)
                mask = (day >= start) & (day < end)
                result.append({'id': f'{split}-{start:04d}', 'split': split,
                               'start_day': start, 'end_day': end,
                               'covariate': float(np.max(np.abs(z[mask].mean(axis=0)-reference))),
                               'confidence': float(abs(score[mask].mean()-reference_score))})
            return result

        dev_windows = windows('development')
        scales = {key: max(1e-12, float(np.quantile([w[key] for w in dev_windows], .8, method='linear')))
                  for key in ('covariate', 'confidence')}
        diagnostic = windows('calibration') + windows('test')
        for w in diagnostic:
            for key in scales:
                w[key] /= scales[key]
        # Freeze probabilities and diagnostics before interpreting any final labels.
        state = {'parameters': model.get_params(), 'diagnostic_scales': scales,
                 'development_reference': reference.tolist(), 'development_score_mean': float(reference_score)}
        if name == 'logistic':
            state.update(coefficients=model.coef_.tolist(), intercept=model.intercept_.tolist(), iterations=model.n_iter_.tolist())
            assert model.n_iter_.max() < 1000, 'unresolved fit convergence'
        else:
            state['trees'] = [{k: getattr(t.tree_, k).tolist() for k in
                               ('children_left', 'children_right', 'feature', 'threshold', 'value')} for t in model.estimators_]
        state_bytes = (json.dumps(state, separators=(',', ':'), allow_nan=False)+'\n').encode()
        (args.output / f'{name}-model.json').write_bytes(state_bytes)
        records = []
        for split in ('calibration', 'test'):
            lo = protocol['split_day_indices_half_open'][split][0]
            for i in np.flatnonzero(splits[split]):
                scenario = 'calibration' if split == 'calibration' else f'test-{lo+7*((int(day[i])-lo)//7):04d}'
                records.append({'id': f'{i:05d}', 'prediction': [int(pred[i])],
                                'truth': [int(data['class'][i] == b'UP')],
                                'uncertainty': float(score[i]), 'error_probability': float(score[i]),
                                'sequence': split+'-dependent-stream', 'spatial_cluster': None,
                                'split': split, 'scenario': scenario})
        inp = {'schema_version': 'triad-metrology-1', 'model_id': name, 'experiment_id': 'electricity-v6-1',
               'loss': 'classification', 'tolerance': 0., 'target_coverage': .8,
               'calibration_scenario': 'calibration', 'bootstrap_replicates': 1000,
               'bootstrap_seed': 61003, 'reliability_bins': 10,
               'evidence': {'numerical_required': False, 'checks': {},
                            'evidence_resources': ['protocol.json', 'metadata.json'],
                            'evaluated_scenarios': [], 'prospective_power_supported': False,
                            'independent_replication': False}, 'records': records}
        manifest_models.append({'model_sha256': digest(state_bytes), 'input': inp, 'windows': diagnostic})
    manifest = {'schema_version': 'triad-benchmark-1', 'provenance': provenance,
                'protocol_sha256': digest(protocol_bytes), 'rows_per_day': 48, 'block_lengths': [1,7,14],
                'alert_quantile': .8, 'degradation_margin': .05, 'models': manifest_models}
    save(args.output / 'manifest.json', manifest)
    save(args.output / 'provenance.json', provenance)
    print('Prepared frozen predictions for two real-data models; no performance-driven selection.')


if __name__ == '__main__':
    main()
