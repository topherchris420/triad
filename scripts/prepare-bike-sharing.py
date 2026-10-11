"""Retrieve UCI Bike Sharing (275), fit the frozen Milestone 7 models and export
the triad-temporal-regression-1 manifest. Never reads benchmark results.

Usage: python scripts/prepare-bike-sharing.py --cache DIRECTORY --output NEW_DIRECTORY

Run from the repository root. All settings come from the committed protocol
(benchmarks/bike-sharing/protocol.json); nothing is tuned. The script prints no
development, calibration or test metric. Output JSON uses canonical LF bytes.
"""
import os

# Single-threaded numerical kernels make summation order platform-stable.
for _var in ('OMP_NUM_THREADS', 'OPENBLAS_NUM_THREADS', 'MKL_NUM_THREADS'):
    os.environ[_var] = '1'

import argparse  # noqa: E402
import csv  # noqa: E402
import datetime as dt  # noqa: E402
import hashlib  # noqa: E402
import importlib.metadata  # noqa: E402
import io  # noqa: E402
import json  # noqa: E402
import platform  # noqa: E402
import urllib.request  # noqa: E402
import zipfile  # noqa: E402
from pathlib import Path  # noqa: E402
from zoneinfo import ZoneInfo  # noqa: E402

import numpy as np  # noqa: E402
from sklearn.ensemble import HistGradientBoostingRegressor  # noqa: E402
from sklearn.linear_model import LogisticRegression, Ridge  # noqa: E402
from sklearn.preprocessing import StandardScaler  # noqa: E402

PROTOCOL = Path('benchmarks/bike-sharing/protocol.json')
ARCHIVE_URL = 'https://archive.ics.uci.edu/static/public/275/bike+sharing+dataset.zip'
NEW_YORK = ZoneInfo('America/New_York')
EPOCH = dt.datetime(2011, 1, 1, 5, tzinfo=dt.timezone.utc)
LAGS = [1, 2, 3, 24, 168]
TOL = (25.0, 0.25)
MANIFEST_SETTINGS = {
    'time_zone_rule': 'America/New_York (US rule since 2007)', 'lags': LAGS,
    'tolerance': {'absolute': TOL[0], 'relative': TOL[1]},
    'tolerance_sensitivity': [{'absolute': 15.0, 'relative': 0.15}, {'absolute': 40.0, 'relative': 0.4}],
    'target_coverage': 0.8, 'reliability_bins': 10, 'conformal_alpha': 0.1, 'degradation_margin': 0.05,
    'min_accepted_per_day': 5, 'alert_quantile': 0.8, 'min_condition_days': 5, 'completeness_required': 22,
    'abstention_costs': [0.25, 0.1, 0.5], 'practical_threshold': 0.01, 'block_lengths': [7, 1, 14],
    'bootstrap_replicates': 2000, 'bootstrap_seed': 71003, 'family_alpha': 0.05, 'equivalence_agreement': 0.95,
}


def digest(data):
    return hashlib.sha256(data).hexdigest()


def canonical(value):
    return (json.dumps(value, separators=(',', ':'), allow_nan=False) + '\n').encode()


def elapsed_hour(day, hour):
    local = dt.datetime.fromisoformat(day).replace(hour=hour, tzinfo=NEW_YORK)
    if local.astimezone(dt.timezone.utc).astimezone(NEW_YORK).replace(tzinfo=None) != local.replace(tzinfo=None):
        raise ValueError(f'non-existent local time {day} {hour}')
    return int((local.astimezone(dt.timezone.utc) - EPOCH).total_seconds() // 3600)


def load(cache, protocol):
    cache.mkdir(parents=True, exist_ok=True)
    path = cache / 'bike-sharing.zip'
    if not path.exists():
        with urllib.request.urlopen(ARCHIVE_URL, timeout=120) as response:
            path.write_bytes(response.read())
    archive = path.read_bytes()
    with zipfile.ZipFile(io.BytesIO(archive)) as z:
        hour = z.read('hour.csv')
    assert digest(hour) == protocol['dataset']['hour_csv_sha256'], 'hour.csv changed; refuse substitution'
    return archive, hour


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--cache', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    protocol_bytes = PROTOCOL.read_bytes()
    protocol = json.loads(protocol_bytes)
    assert protocol['seeds'] == {'hgb': 71002, 'bootstrap': 71003}
    assert protocol['models']['hgb']['random_state'] == 71002 and protocol['selective']['target_coverage'] == 0.8
    archive, hour_bytes = load(args.cache, protocol)
    rows = list(csv.DictReader(io.StringIO(hour_bytes.decode('ascii'))))
    n = len(rows)
    cnt = np.array([int(r['cnt']) for r in rows], dtype=float)
    identity = max(abs(int(r['casual']) + int(r['registered']) - int(r['cnt'])) for r in rows)
    elapsed = np.array([elapsed_hour(r['dteday'], int(r['hr'])) for r in rows])
    assert np.all(np.diff(elapsed) > 0), 'elapsed hours must be strictly increasing'
    position = {int(e): i for i, e in enumerate(elapsed)}
    dates = [dt.date.fromisoformat(r['dteday']) for r in rows]
    hour = np.array([int(r['hr']) for r in rows])
    holiday = np.array([int(r['holiday']) for r in rows])
    working = np.array([int(r['workingday']) for r in rows])
    weekday = np.array([int(r['weekday']) for r in rows])
    doy = np.array([d.timetuple().tm_yday for d in dates], dtype=float)
    lag_index = np.full((n, len(LAGS)), -1)
    for i, e in enumerate(elapsed):
        for k, lag in enumerate(LAGS):
            lag_index[i, k] = position.get(int(e) - lag, -1)
    eligible = (lag_index >= 0).all(axis=1)
    windows = {k: v for k, v in protocol['partitions_by_local_date_of_T_inclusive'].items() if k != 'rules'}
    bounds = {k: (dt.date.fromisoformat(a), dt.date.fromisoformat(b)) for k, (a, b) in windows.items()}
    part = np.array([next(k for k, (a, b) in bounds.items() if a <= d <= b) for d in dates])
    masks = {k: eligible & (part == k) for k in bounds}
    # Information set: calendar of T and counts strictly before T (elapsed lags).
    lags = np.where(lag_index >= 0, np.log1p(cnt[np.clip(lag_index, 0, None)]), np.nan)
    doy_sin, doy_cos = np.sin(2 * np.pi * doy / 365.25), np.cos(2 * np.pi * doy / 365.25)
    ridge_x = np.column_stack(
        [(hour == h) & (working == w) for w in (1, 0) for h in range(24)]
        + [holiday] + [weekday == d for d in range(7)] + [doy_sin, doy_cos, lags]).astype(float)
    hgb_x = np.column_stack([hour, working, holiday, weekday, doy_sin, doy_cos, lags]).astype(float)
    target = np.log1p(cnt)
    train = masks['training']
    scaler = StandardScaler().fit(ridge_x[train])
    ridge = Ridge(**{k: protocol['models']['ridge'][k] for k in ('alpha', 'solver')}).fit(scaler.transform(ridge_x[train]), target[train])
    hgb_params = {k: protocol['models']['hgb'][k] for k in ('loss', 'max_iter', 'learning_rate', 'max_leaf_nodes', 'min_samples_leaf',
                                                           'l2_regularization', 'max_bins', 'early_stopping', 'random_state')}
    hgb = HistGradientBoostingRegressor(**hgb_params).fit(hgb_x[train], target[train])
    predict = {'ridge': lambda m: ridge.predict(scaler.transform(ridge_x[m])),
               'hgb': lambda m: hgb.predict(hgb_x[m])}
    used = eligible & (part != 'training')
    args.output.mkdir(parents=True, exist_ok=False)
    manifest_models = []
    states = {}
    for name in ('ridge', 'hgb'):
        yhat = np.zeros(n)
        yhat[used] = np.maximum(0.0, np.expm1(predict[name](used)))
        dev = masks['development']
        scale_x = np.column_stack([np.log1p(yhat), np.sin(2 * np.pi * hour / 24), np.cos(2 * np.pi * hour / 24),
                                   np.sin(4 * np.pi * hour / 24), np.cos(4 * np.pi * hour / 24), working, holiday,
                                   np.abs(lags[:, 0] - lags[:, 1])])
        scale_scaler = StandardScaler().fit(scale_x[dev])
        scale_model = Ridge(alpha=1.0, solver='cholesky').fit(scale_scaler.transform(scale_x[dev]), np.log1p(np.abs(cnt[dev] - yhat[dev])))
        s = np.ones(n)
        s[used] = np.maximum(1.0, np.expm1(scale_model.predict(scale_scaler.transform(scale_x[used]))))
        event = np.abs(yhat - cnt) > np.maximum(TOL[0], TOL[1] * cnt)
        prob_x = np.column_stack([np.log(s), np.log1p(yhat)])
        # Development labels only; no hyperparameters, no test information.
        prob_model = LogisticRegression(penalty=None, solver='lbfgs', max_iter=1000).fit(prob_x[dev], event[dev].astype(int))
        assert prob_model.n_iter_.max() < 1000, 'error-probability fit did not converge'
        p = prob_model.predict_proba(prob_x)[:, 1]
        state = {'model': name, 'parameters': {k: v for k, v in (ridge if name == 'ridge' else hgb).get_params().items()},
                 'error_scale_model': {'scaler_mean': scale_scaler.mean_.tolist(), 'scaler_scale': scale_scaler.scale_.tolist(),
                                       'coefficients': scale_model.coef_.tolist(), 'intercept': float(scale_model.intercept_)},
                 'error_probability_model': {'coefficients': prob_model.coef_.tolist(), 'intercept': prob_model.intercept_.tolist(),
                                             'iterations': prob_model.n_iter_.tolist()}}
        if name == 'ridge':
            state.update(scaler_mean=scaler.mean_.tolist(), scaler_scale=scaler.scale_.tolist(),
                         coefficients=ridge.coef_.tolist(), intercept=float(ridge.intercept_))
        else:
            state['baseline_prediction'] = np.asarray(hgb._baseline_prediction).ravel().tolist()
            state['n_iter'] = int(hgb.n_iter_)
            state['trees'] = [{f: tree.nodes[f].tolist() for f in ('value', 'feature_idx', 'num_threshold', 'left', 'right', 'is_leaf', 'missing_go_to_left')}
                              for (tree,) in hgb._predictors]
        state_bytes = canonical(state)
        (args.output / f'{name}-model.json').write_bytes(state_bytes)
        states[name] = state
        predictions = [{'date': rows[i]['dteday'], 'hour': int(hour[i]), 'prediction': float(yhat[i]),
                        'error_scale': float(s[i]), 'error_probability': float(p[i])} for i in np.flatnonzero(used)]
        manifest_models.append({'model_id': name, 'model_sha256': digest(state_bytes), 'predictions': predictions})
    software = {k: importlib.metadata.version(k) for k in ('numpy', 'scipy', 'scikit-learn', 'joblib', 'threadpoolctl')}
    provenance = {
        'source_url': ARCHIVE_URL, 'doi': protocol['dataset']['doi'], 'uci_id': 275,
        'sha256': digest(hour_bytes), 'archive_sha256': digest(archive),
        'archive_matches_recorded': digest(archive) == protocol['dataset']['archive_sha256_observed_2026_10_11'],
        'license': protocol['dataset']['license'], 'citation': protocol['dataset']['citation'],
        'redistribution': 'The manifest contains derived hourly fields (date, hour, holiday, workingday, weekday, cnt) under CC BY 4.0 with attribution; the raw archive is retrieved, not committed.',
        'rows': n, 'cnt_minus_casual_plus_registered_max_abs': identity,
        'eligible_by_partition': {k: int(v.sum()) for k, v in masks.items()},
        'present_by_partition': {k: int((part == k).sum()) for k in bounds},
        'software': software, 'python': platform.python_version(), 'threads': 'OMP/OpenBLAS/MKL = 1',
        'preparation_source_sha256': digest(Path(__file__).read_bytes()),
        'protocol_sha256': digest(protocol_bytes),
        'feature_matrices_sha256': {'ridge_training_standardised': digest(np.asarray(scaler.transform(ridge_x[train]), dtype='<f8').tobytes()),
                                    'hgb_training': digest(np.asarray(hgb_x[train], dtype='<f8').tobytes())},
    }
    hours = [{'date': r['dteday'], 'hour': int(r['hr']), 'holiday': int(r['holiday']), 'workingday': int(r['workingday']),
              'weekday': int(r['weekday']), 'cnt': int(r['cnt'])} for r in rows]
    manifest = {'schema_version': 'triad-temporal-regression-1', 'benchmark_id': protocol['version'],
                'provenance': provenance, 'protocol_sha256': digest(protocol_bytes),
                'partitions': {k: list(v) for k, v in windows.items()},
                **{k: v for k, v in MANIFEST_SETTINGS.items()}, 'hours': hours, 'models': manifest_models}
    order = ['schema_version', 'benchmark_id', 'provenance', 'protocol_sha256', 'time_zone_rule', 'partitions'] + \
        [k for k in MANIFEST_SETTINGS if k != 'time_zone_rule'] + ['hours', 'models']
    manifest = {k: manifest[k] for k in order}
    (args.output / 'manifest.json').write_bytes(canonical(manifest))
    (args.output / 'provenance.json').write_bytes(canonical(provenance))
    (args.output / 'protocol.json').write_bytes(protocol_bytes)
    print(f'Prepared frozen predictions for {len(manifest_models)} models; no performance-driven selection; no metrics printed.')


if __name__ == '__main__':
    main()
