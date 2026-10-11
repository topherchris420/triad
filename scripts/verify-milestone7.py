"""Independent NumPy/SciPy recomputation of the Milestone 7 Rust reports.

Usage (from the repository root):
  python scripts/verify-milestone7.py --bike REPORT.json --suite SUITE_REPORT.json
         [--evidence EVIDENCE_REPORT.json] [--data CACHE_DIR] [--plots DIR]

No Rust code is imported or called. Elapsed time uses Python zoneinfo, the
Kolmogorov-Smirnov statistic uses scipy.stats.ks_2samp, Clopper-Pearson bounds
use scipy.stats.beta.ppf, and the evidence rules are re-implemented from
docs/metrology/evidence-sufficiency-method.md. Computational cross-check only;
not scientific replication. Tolerance: relative 1e-9, absolute 1e-11.
"""
import argparse
import datetime as dt
import hashlib
import io
import json
import math
import zipfile
from pathlib import Path
from zoneinfo import ZoneInfo

import numpy as np
from scipy import stats

NY = ZoneInfo('America/New_York')
EPOCH = dt.datetime(2011, 1, 1, 5, tzinfo=dt.timezone.utc)
CHECKS = 0


def close(actual, expected, what=''):
    global CHECKS
    CHECKS += 1
    if expected is None or actual is None:
        assert actual is None and expected is None, (what, actual, expected)
        return
    np.testing.assert_allclose(actual, expected, rtol=1e-9, atol=1e-11, err_msg=what)


def same(actual, expected, what=''):
    global CHECKS
    CHECKS += 1
    assert actual == expected, (what, actual, expected)


def elapsed(date, hour):
    local = dt.datetime.fromisoformat(date).replace(hour=hour, tzinfo=NY)
    return int((local.astimezone(dt.timezone.utc) - EPOCH).total_seconds() // 3600)


def draws(seed, n):
    mask = (1 << 64) - 1
    bound = ((1 << 64) - n) % n
    while True:
        seed = (seed + 0x9e3779b97f4a7c15) & mask
        z = seed
        z = ((z ^ (z >> 30)) * 0xbf58476d1ce4e5b9) & mask
        z = ((z ^ (z >> 27)) * 0x94d049bb133111eb) & mask
        z ^= z >> 31
        if z >= bound:
            yield z % n


def resamples(days, block, reps, seed):
    if days // block < 2:
        return None
    gen = draws(seed, days)
    out = np.empty((reps, days), dtype=int)
    for r in range(reps):
        idx = []
        while len(idx) < days:
            start = next(gen)
            idx.extend((start + np.arange(min(block, days - len(idx)))) % days)
        out[r] = idx
    return out


def ratio_interval(diff, n, idx, level):
    if idx is None:
        return None
    num, den = diff[idx].sum(axis=1), n[idx].sum(axis=1)
    assert (den > 0).all()
    a = (1.0 - level) / 2.0
    return np.quantile(num / den, [a, 1 - a], method='linear').tolist()


def cp_lower(k, n, gamma):
    return 0.0 if k <= 0 else float(stats.beta.ppf(1 - gamma, k, n - k + 1))


def cp_upper(k, n, gamma):
    return 1.0 if k >= n else float(stats.beta.ppf(gamma, k + 1, n - k))


# --------------------------------------------------------------------------- generic selective metrics

def operational(score, event, tau):
    acc = score <= tau if tau is not None else np.zeros(len(score), bool)
    n, a = len(score), int(acc.sum())
    return {'count': n, 'accepted': a, 'errors': int(event.sum()), 'accepted_errors': int(event[acc].sum()),
            'coverage': a / n, 'abstention_frequency': 1 - a / n,
            'selective_error_risk': float(event[acc].mean()) if a else None,
            'incorrect_acceptance_frequency': event[acc].sum() / n, 'error_rate': event.mean()}


def check_generic(actual, score, event, ids, tau, bins, target):
    op = operational(score, event, tau)
    for k, v in op.items():
        close(actual['operational'][k], v, k)
    order = np.lexsort((ids, score))
    k = int(math.floor(target * len(score)))
    retro = np.zeros(len(score), bool)
    retro[order[:k]] = True
    same(actual['retrospective_matched_coverage']['accepted_errors'], int(event[retro].sum()), 'retrospective')
    s = score[order]
    ends = np.r_[np.flatnonzero(np.diff(s) != 0) + 1, len(s)]
    risks = np.cumsum(event[order])[ends - 1] / ends
    close(actual['aurc'], float(np.sum(np.diff(np.r_[0, ends]) / len(s) * risks)), 'aurc')
    same(len(actual['risk_coverage_curve']), len(ends), 'curve length')
    close(actual['brier'], float(np.mean((score - event) ** 2)), 'brier')
    assign = np.minimum(np.floor(score * bins).astype(int), bins - 1)
    close(actual['ece'], sum(abs(score[assign == b].sum() - event[assign == b].sum()) for b in range(bins)) / len(s), 'ece')


# --------------------------------------------------------------------------- bike benchmark

def errors(yhat, y, tol):
    if len(y) == 0:
        return {'n': 0}
    ev = np.abs(yhat - y) > np.maximum(tol[0], tol[1] * y)
    return {'n': len(y), 'mae': np.mean(np.abs(yhat - y)), 'rmse': math.sqrt(np.mean((yhat - y) ** 2)),
            'bias': np.mean(yhat - y), 'material_errors': int(ev.sum()), 'material_error_rate': ev.mean()}


def check_errors(actual, expected, what):
    for k, v in expected.items():
        close(actual[k], v, f'{what}.{k}')


def verify_bike(path, data_dir):
    report = json.loads(Path(path).read_bytes())['report']
    m = report['manifest']
    hours = m['hours']
    el = np.array([elapsed(h['date'], h['hour']) for h in hours])
    assert np.all(np.diff(el) > 0)
    cnt = np.array([h['cnt'] for h in hours], float)
    where = {int(e): i for i, e in enumerate(el)}
    for h in hours:
        d = dt.date.fromisoformat(h['date'])
        assert h['weekday'] == (d.weekday() + 1) % 7 and h['workingday'] == int(d.weekday() < 5 and h['holiday'] == 0)
    if data_dir:
        with zipfile.ZipFile(Path(data_dir) / 'bike-sharing.zip') as z:
            raw = z.read('hour.csv')
        same(hashlib.sha256(raw).hexdigest(), m['provenance']['sha256'], 'hour.csv sha256')
        import csv
        src = list(csv.DictReader(io.StringIO(raw.decode('ascii'))))
        same([(r['dteday'], int(r['hr']), int(r['holiday']), int(r['workingday']), int(r['weekday']), int(r['cnt'])) for r in src],
             [(h['date'], h['hour'], h['holiday'], h['workingday'], h['weekday'], h['cnt']) for h in hours], 'manifest equals source')
        same(max(abs(int(r['casual']) + int(r['registered']) - int(r['cnt'])) for r in src), 0, 'cnt identity')
    for model in m['models']:
        same(hashlib.sha256(Path(f"benchmarks/bike-sharing/{model['model_id']}-model.json").read_bytes()).hexdigest(), model['model_sha256'], 'model hash')
    same(hashlib.sha256(Path('benchmarks/bike-sharing/protocol.json').read_bytes()).hexdigest(), m['protocol_sha256'], 'protocol hash')
    lags = m['lags']
    dates = np.array([dt.date.fromisoformat(h['date']).toordinal() for h in hours])
    parts = {k: (dt.date.fromisoformat(a).toordinal(), dt.date.fromisoformat(b).toordinal()) for k, (a, b) in m['partitions'].items()}
    eligible = np.array([all(int(e) - l in where for l in lags) for e in el])
    for name, (a, b) in parts.items():
        in_part = (dates >= a) & (dates <= b)
        span = elapsed(dt.date.fromordinal(b + 1).isoformat(), 0) - elapsed(dt.date.fromordinal(a).isoformat(), 0)
        d = report['data']['partitions'][name]
        same(d['eligible'], int((in_part & eligible).sum()), f'{name} eligible')
        same(d['present_target_hours'], int(in_part.sum()), f'{name} present')
        same(d['missing_target_hours'], span - int(in_part.sum()), f'{name} missing')
    used = np.flatnonzero(eligible & (dates >= parts['development'][0]))
    part = np.where(dates[used] <= parts['development'][1], 'development',
                    np.where(dates[used] <= parts['calibration'][1], 'calibration', 'test'))
    holiday_days = {dates[i] for i in range(len(hours)) if hours[i]['holiday'] == 1}

    def day_type(o):
        if o in holiday_days:
            return 'holiday'
        return 'weekend' if dt.date.fromordinal(o).weekday() >= 5 else 'working day'

    tol = (m['tolerance']['absolute'], m['tolerance']['relative'])
    y = cnt[used]
    lagv = np.array([[cnt[where[int(el[i]) - l]] for l in lags] for i in used])
    rep_iter = iter(report['models'])
    plots = []
    for model in m['models']:
        rep = next(rep_iter)
        pred = model['predictions']
        same([(p['date'], p['hour']) for p in pred], [(hours[i]['date'], hours[i]['hour']) for i in used], 'prediction keys')
        yhat = np.array([p['prediction'] for p in pred])
        s = np.array([p['error_scale'] for p in pred])
        p = np.array([p['error_probability'] for p in pred])
        test, cal, dev = part == 'test', part == 'calibration', part == 'development'
        conv = rep['conventional']
        for name, mask in (('development', dev), ('calibration', cal), ('test', test)):
            check_errors(conv[name], errors(yhat[mask], y[mask], tol), name)
        month = np.array([dt.date.fromordinal(o).strftime('%Y-%m') for o in dates[used]])
        dtypes = np.array([day_type(o) for o in dates[used]])
        for mo in sorted(set(month[test])):
            check_errors(conv['test_by_month'][mo], errors(yhat[test & (month == mo)], y[test & (month == mo)], tol), mo)
        for k in (1, 24, 168):
            check_errors(rep['reference_forecasts_test'][f'cnt(T-{k})'], errors(lagv[test, lags.index(k)], y[test], tol), f'ref {k}')
        # Generic selective metrics on the material-error event.
        event = np.abs(yhat - y) > np.maximum(tol[0], tol[1] * y)
        cs = np.sort(p[cal])
        n_acc = int(math.ceil(m['target_coverage'] * len(cs)))
        tau = float(cs[n_acc - 1]) if n_acc else None
        close(rep['selective_threshold'], tau, 'tau')
        gen = rep['selective_generic']
        ids = np.array([f'{int(e):06d}' for e in el[used]])
        check_generic(gen['overall'], p[test], event[test].astype(float), ids[test], tau, m['reliability_bins'], m['target_coverage'])
        for mo in sorted(set(month[test])):
            sel = test & (month == mo)
            check_generic(gen['scenarios'][f'test-{mo}'], p[sel], event[sel].astype(float), ids[sel], tau, m['reliability_bins'], m['target_coverage'])
        same(gen['overall']['sequence_bootstrap']['status'], 'degenerate_or_insufficient_evidence', 'one dependent stream')
        accepted = p <= tau
        # Conformal.
        sc = np.sort(np.abs(y[cal] - yhat[cal]) / s[cal])
        kq = int(math.ceil((len(sc) + 1) * (1 - m['conformal_alpha'])))
        q = float(sc[kq - 1]) if 1 <= kq <= len(sc) else None
        close(rep['conformal']['quantile'], q, 'conformal q')
        covered = np.abs(y - yhat) <= q * s
        same(rep['conformal']['test']['covered'], int(covered[test].sum()), 'conformal covered')
        close(rep['conformal']['test']['mean_width_bikes'], float(np.mean(2 * q * s[test])), 'width')
        for mo in sorted(set(month[test])):
            same(rep['conformal']['test_by_month'][mo]['covered'], int(covered[test & (month == mo)].sum()), mo)
        mon = verify_monitoring(rep['monitoring'], m, tol, el[used], dates[used], part, y, yhat, p, lagv, accepted, day_type, parts, detail=True)
        for st, actual in zip(m['tolerance_sensitivity'], rep['tolerance_sensitivity']):
            t2 = (st['absolute'], st['relative'])
            check_errors(actual['test_errors'], errors(yhat[test], y[test], t2), 'sensitivity errors')
            ev2 = np.abs(yhat - y) > np.maximum(t2[0], t2[1] * y)
            for k, v in operational(p[test], ev2[test].astype(float), tau).items():
                close(actual['selective'][k], v, 'sensitivity ' + k)
            sub = verify_monitoring({'primary': actual['primary'], 'r_cal': actual['r_cal'], 'triad_test_alert_days': actual['triad_test_alert_days'], 'degraded_days': actual['degraded_days']},
                                    m, t2, el[used], dates[used], part, y, yhat, p, lagv, accepted, day_type, parts, detail=False)
            assert sub is None
        plots.append((rep['model_id'], gen, rep, mon))
    conf = report['confirmatory']
    same(conf['added_predictive_utility_established'], all(c['decision'] == 'advantage_established' for c in conf['per_model']), 'overall decision')
    return report, plots


def verify_monitoring(actual, m, tol, el, dates, part, y, yhat, p, lagv, accepted, day_type, parts, detail):
    event = np.abs(yhat - y) > np.maximum(tol[0], tol[1] * y)
    cal = part == 'calibration'
    r_cal = event[cal & accepted].sum() / (cal & accepted).sum()
    close(actual['r_cal'], r_cal, 'r_cal')
    bound = min(r_cal + m['degradation_margin'], 1.0)
    pos = {int(e): i for i, e in enumerate(el)}
    pairs = np.array([(event[i], event[pos[int(e) - 1]]) for i, e in enumerate(el)
                      if cal[i] and int(e) - 1 in pos and cal[pos[int(e) - 1]]], float)
    rho = np.corrcoef(pairs[:, 0], pairs[:, 1])[0, 1] if pairs[:, 0].std() > 0 and pairs[:, 1].std() > 0 else 0.0
    deff = (1 + min(max(rho, 0.0), 0.95)) / (1 - min(max(rho, 0.0), 0.95))
    if detail:
        close(actual['lag1_event_autocorrelation_calibration'], rho, 'rho')
        close(actual['design_effect'], deff, 'deff')
    dev = part == 'development'
    first, last = parts['development'][0], parts['test'][1]
    ordinals = list(range(first, last + 1))
    by_day = {o: np.flatnonzero(dates == o) for o in ordinals}
    reference = [lagv[dev, k] for k in range(lagv.shape[1])]

    def prev_scores(o):
        rows = by_day[o - 1]
        acc = rows[accepted[rows]]
        ks = max(stats.ks_2samp(lagv[rows, k], reference[k]).statistic for k in range(lagv.shape[1])) if len(rows) else None
        return {'cov': ks, 'conf': float(p[rows].mean()) if len(rows) else None,
                'perf': float(event[acc].mean()) if len(acc) else 0.0, 'available': len(rows),
                'recent': event[acc]}

    span = lambda name: [o for o in range(parts[name][0], parts[name][1] + 1) if o - 1 >= first]  # noqa: E731
    dev_s = [prev_scores(o) for o in span('development')]
    cal_days, test_days = span('calibration'), span('test')
    cal_s = [prev_scores(o) for o in cal_days]
    test_s = [prev_scores(o) for o in test_days]
    floor = lambda v: max(1e-12, float(np.quantile(v, 0.8, method='linear')) if len(v) else 0.0)  # noqa: E731
    scale = (floor([s['cov'] for s in dev_s if s['cov'] is not None]), floor([s['perf'] for s in dev_s]))
    if detail:
        close(actual['score_scales_development_p80']['cov'], scale[0], 'scale cov')
        close(actual['score_scales_development_p80']['perf'], scale[1], 'scale perf')
    counts = {}
    for o in cal_days:
        counts[day_type(o)] = counts.get(day_type(o), 0) + 1
    support_max = max(s['cov'] for s in cal_s if s['cov'] is not None)

    def triad(o, s, off=()):
        insufficient = contradicted = False
        if 'r1' not in off and counts.get(day_type(o), 0) < m['min_condition_days']:
            insufficient = True
        if 'r2' not in off and (s['cov'] is None or s['cov'] > support_max):
            insufficient = True
        if 'r3' not in off and s['available'] < m['completeness_required']:
            insufficient = True
        if 'r4' not in off and len(s['recent']):
            k, n = s['recent'].sum() / deff, len(s['recent']) / deff
            if cp_lower(k, n, 0.95) > bound:
                contradicted = True
        return insufficient or contradicted

    k_cal = sum(triad(o, s) for o, s in zip(cal_days, cal_s))
    policies = {'none': [False] * len(test_days), 'triad': [bool(triad(o, s)) for o, s in zip(test_days, test_s)]}
    k_test = sum(policies['triad'])
    cal_alerts = {'triad': k_cal}
    k_outcome_free = 0
    for name, off in (('triad_without_r1_calibration_applicability', 'r1'), ('triad_without_r2_representativeness', 'r2'),
                      ('triad_without_r3_completeness', 'r3'), ('triad_without_r4_recent_outcomes', 'r4')):
        policies[name] = [bool(triad(o, s, (off,))) for o, s in zip(test_days, test_s)]
        cal_alerts[name] = sum(triad(o, s, (off,)) for o, s in zip(cal_days, cal_s))
        if off == 'r4':
            k_outcome_free = cal_alerts[name]

    def score(name, s):
        if name == 'std':
            return max((s['cov'] or 0.0) / scale[0], s['perf'] / scale[1])
        return s[name]

    def alerts(v, scores, extra):
        return [bool(e or (x is not None and x > v)) for x, e in zip(scores, extra)]

    def matched(scores, extra, k):
        for v in sorted(x for x in scores if x is not None):
            if sum(alerts(v, scores, extra)) <= k:
                return v
        return max(x for x in scores if x is not None)

    for name in ('cov', 'conf', 'perf', 'std'):
        cs = [score(name, s) for s in cal_s]
        ts = [score(name, s) for s in test_s]
        ce = [name == 'std' and s['available'] < m['completeness_required'] for s in cal_s]
        te = [name == 'std' and s['available'] < m['completeness_required'] for s in test_s]
        native = float(np.quantile([x for x in cs if x is not None], m['alert_quantile'], method='linear'))
        mt = matched(cs, ce, k_cal)
        if detail:
            close(actual['thresholds'][name]['native'], native, name + ' native')
            close(actual['thresholds'][name]['matched_calibration'], mt, name + ' matched')
        policies[f'{name}_native'] = alerts(native, ts, te)
        policies[f'{name}_matched_calibration'] = alerts(mt, ts, te)
        order = sorted(range(len(ts)), key=lambda i: (-(ts[i] if ts[i] is not None else -np.inf), i))
        top = [False] * len(ts)
        for i in order[:k_test]:
            top[i] = True
        policies[f'{name}_matched_test_topk'] = top
        if name in ('cov', 'conf'):
            policies[f'{name}_matched_outcome_free'] = alerts(matched(cs, ce, k_outcome_free), ts, te)
    same(actual['triad_test_alert_days'], k_test, 'triad test alerts')
    rows = [by_day[o] for o in test_days]
    acc_day = [int(accepted[r].sum()) for r in rows]
    err_day = [int((accepted[r] & event[r]).sum()) for r in rows]
    degraded = [bool(e / a > bound) if a >= m['min_accepted_per_day'] else None for a, e in zip(acc_day, err_day)]
    dd = actual['degraded_days']
    same(dd['degraded'], sum(d is True for d in degraded), 'degraded days')
    same(dd['known'], sum(d is not None for d in degraded), 'known days')
    n_day = np.array([len(r) for r in rows], float)

    def costs(alert, c):
        return np.array([np.where(accepted[r] & (not a), event[r].astype(float), c).sum() for r, a in zip(rows, alert)])

    if detail:
        same(actual['triad_calibration_alert_days_k'], k_cal, 'K')
        same(sorted(actual['policies']), sorted(policies), 'policy names')
        all_test = np.concatenate(rows)
        for name, alert in policies.items():
            pr = actual['policies'][name]
            same(pr['test_alert_days'], sum(alert), name + ' alerts')
            if name in cal_alerts:
                same(pr['calibration_alert_days'], cal_alerts[name], name + ' calibration alerts')
            keep = np.concatenate([accepted[r] & (not a) for r, a in zip(rows, alert)])
            ev = np.concatenate([event[r] for r in rows])
            same(pr['gated']['accepted'], int(keep.sum()), name + ' gated')
            same(pr['gated']['accepted_errors'], int(ev[keep].sum()), name + ' gated errors')
            k = int(keep.sum())
            ranked = all_test[np.lexsort((el[all_test], p[all_test]))][:k]
            same(pr['per_hour_threshold_at_same_coverage']['accepted_errors'], int(event[ranked].sum()), name + ' hourly')
            conf = [0] * 6
            for d, a in zip(degraded, alert):
                conf[{(True, True): 0, (False, True): 1, (True, False): 2, (False, False): 3, (None, True): 4, (None, False): 5}[(d, a)]] += 1
            same(pr['degraded_day_confusion'], dict(zip(('true_positive', 'false_positive', 'false_negative', 'true_negative', 'unknown_alerted', 'unknown_not_alerted'), conf)), name + ' confusion')
            delays, i = [], 0
            while i < len(degraded):
                if degraded[i] is True:
                    j = i
                    while j < len(degraded) and degraded[j] is True:
                        j += 1
                    delays.append(next((t - i for t in range(i, j) if alert[t]), None))
                    i = j
                else:
                    i += 1
            same(pr['episode_delay_days'], delays, name + ' delays')
            for cv in pr['costs']:
                close(cv['mean_cost'], costs(alert, cv['abstention_cost']).sum() / n_day.sum(), name + ' cost')
    blocks = {b: resamples(len(test_days), b, m['bootstrap_replicates'], m['bootstrap_seed']) for b in m['block_lengths']}
    level = 1.0 - m['family_alpha'] / len(m['models'])

    def compare(entry, a, b, levels, block_list):
        c = entry['abstention_cost']
        diff = costs(policies[a], c) - costs(policies[b], c)
        close(entry['delta_mean_cost_a_minus_b'], diff.sum() / n_day.sum(), f'delta {a} {b}')
        same(entry['identical_decision_days'], sum(x == y for x, y in zip(policies[a], policies[b])), 'identical days')
        for bl, got in zip(block_list, entry['bootstrap']):
            same(got['block_days'], bl, 'block')
            for lv, iv in zip(levels, got['intervals']):
                close(iv['level'], lv, 'level')
                close(iv['interval'], ratio_interval(diff, n_day, blocks[bl], lv), f'interval {a} {b} {bl} {lv}')
        return diff.sum() / n_day.sum()

    prim = actual['primary']
    delta = compare(prim['comparison'], 'triad', 'std_matched_calibration', [level, 0.95], m['block_lengths'])
    lo, hi = prim['comparison']['bootstrap'][0]['intervals'][0]['interval']
    identical = prim['comparison']['identical_decision_days'] == len(test_days)
    decision = ('identical_decisions' if identical else 'advantage_established' if delta <= -m['practical_threshold'] and hi < 0
                else 'disadvantage_established' if delta >= m['practical_threshold'] and lo > 0
                else 'evidence_insufficient_to_establish_a_practical_advantage')
    same(prim['decision'], decision, 'primary decision')
    if not detail:
        return None
    for entry in actual['secondary_comparisons']:
        compare(entry, entry['a'], entry['b'], [0.95], m['block_lengths'][:1])
    for entry in actual['amendment_1_outcome_free']['comparisons']:
        compare(entry, entry['a'], entry['b'], [0.95], m['block_lengths'][:1])
    for ag in actual['agreement'] + actual['amendment_1_outcome_free']['agreement']:
        x, z = np.array(policies[ag['a']]), np.array(policies[ag['b']])
        po = float((x == z).mean())
        pe = x.mean() * z.mean() + (1 - x.mean()) * (1 - z.mean())
        close(ag['agreement'], po, 'agreement')
        close(ag['cohen_kappa'], (po - pe) / (1 - pe) if pe < 1 else None, 'kappa')
    for day, got in zip(test_days, actual['test_days']):
        same(got['date'], dt.date.fromordinal(day).isoformat(), 'day order')
    return {'days': test_days, 'degraded': degraded, 'policies': policies, 'acc': acc_day, 'err': err_day, 'bound': bound}


# --------------------------------------------------------------------------- evidence rules (independent)

AUTH = {'self_reported': 0, 'repository_record': 1, 'external_source': 2}
FAMILIES = ['integrity', 'scope', 'dependence', 'statistical', 'calibration_applicability', 'representativeness', 'completeness',
            'attestation_policy', 'authority', 'leakage_mechanics', 'limitations', 'universality', 'applicability']


def file_ok(e, root):
    if e['kind'] == 'embedded_artifact':
        return hashlib.sha256(e['content'].encode()).hexdigest() == e['expected_sha256']
    path = root / e['path']
    if not path.exists():
        return None
    return hashlib.sha256(path.read_bytes()).hexdigest() == e['expected_sha256']


def rate(e):
    if e['kind'] == 'outcome_records':
        return dict(n=len(e['outcomes']), k=sum(e['outcomes']), v=None, dep=e['dependence'], iv=None, deff=e['design_effect'], cond=e['conditions'], id=e['id'])
    return dict(n=e['n'], k=e['events'], v=e['value'], dep=e['dependence'], iv=e['interval'], deff=e['design_effect'], cond=e['conditions'], id=e['id'])


def point(r):
    if r['k'] is not None:
        if r['k'] > r['n'] or r['n'] <= 0:
            return None
        if r['v'] is not None and abs(r['k'] / r['n'] - r['v']) > 1e-9:
            return None
        return r['k'] / r['n']
    return r['v']


def bounds(r, gamma, honour):
    iv = r['iv']
    aware = iv is not None and iv['method'] in ('block_bootstrap', 'cluster_bootstrap')
    if iv is not None and (not honour or r['dep'] == 'independent' or aware) and iv['level'] >= gamma:
        return iv['lower'], iv['upper']
    if r['k'] is None or r['k'] > r['n'] or r['n'] <= 0:
        return None
    if honour and (r['dep'] == 'unknown' or (r['dep'] in ('temporal', 'clustered') and r['deff'] is None)):
        return None
    d = (r['deff'] or 1.0) if honour else 1.0
    return cp_lower(r['k'] / d, r['n'] / d, gamma), cp_upper(r['k'] / d, r['n'] / d, gamma)


def triad_class(a, root, off=frozenset()):
    claim = a['claims'][0]
    ev = {e['id']: e for e in a['evidence']}
    linked = [ev[i] for i in claim['evidence']]
    on = lambda f: f not in off  # noqa: E731
    out = []  # outcomes: S support, I insufficient, C contradict, N not applicable, O note

    def kinds(*k):
        return [e for e in linked if e['kind'] in k]

    integrity_failed = False
    t = claim['type']
    if t != 'provenance' and on('integrity'):
        for e in kinds('file_hash', 'embedded_artifact'):
            if file_ok(e, root) is not True:
                integrity_failed = True
                out.append('I')
    rates = lambda pop=True: [rate(e) for e in kinds('metric_report', 'outcome_records')  # noqa: E731
                              if e['metric'] == claim.get('metric') and (not pop or e['population'] == claim['population'])]

    def dependence(r):
        if not on('dependence'):
            return
        if r['dep'] == 'unknown':
            out.append('I')
        elif r['dep'] in ('temporal', 'clustered') and not (r['iv'] and r['iv']['method'] in ('block_bootstrap', 'cluster_bootstrap')) and r['deff'] is None:
            out.append('I')
        elif r['dep'] == 'independent':
            for d in kinds('dependence_diagnostic'):
                if abs(d['lag1_autocorrelation']) > 1.959963984540054 / math.sqrt(d['n']):
                    out.append('I')

    def bound_rule(r, cmp, b, g):
        if not on('statistical'):
            v = point(r)
            out.append('I' if v is None else 'S' if (v <= b if cmp == 'at_most' else v >= b) else 'C')
            return
        bb = bounds(r, g, on('dependence'))
        if bb is None:
            out.append('I')
            return
        lo, hi = bb
        sup, con = (hi <= b, lo > b) if cmp == 'at_most' else (lo >= b, hi < b)
        out.append('S' if sup else 'C' if con else 'I')

    if t in ('metric_value', 'metric_bound'):
        rs = rates()
        if not rs:
            out.append('I')
        for r in rs:
            if on('scope') and any(c not in r['cond'] for c in claim['conditions']):
                out.append('I')
            v = point(r)
            if v is None:
                out.append('I')
                continue
            if t == 'metric_value':
                cv, tol, cmp = claim['claimed_value'], claim['tolerance'], claim['comparator']
                ok = abs(v - cv) <= tol if cmp == 'equals' else v <= cv + tol if cmp == 'at_most' else v >= cv - tol
                out.append('S' if ok else 'C')
            else:
                if claim['claimed_value'] is not None and abs(v - claim['claimed_value']) > claim['tolerance']:
                    out.append('C')
                dependence(r)
                bound_rule(r, claim['comparator'], claim['bound'], claim['confidence'])
    elif t == 'coverage_guarantee':
        cals = kinds('calibration')
        if not cals:
            out.append('I')
        else:
            c = cals[0]
            out.append('S')
            if on('calibration_applicability'):
                if any(x not in c['conditions'] or c['condition_counts'].get(x, 0) == 0 for x in claim['conditions']):
                    out.append('I')
                if c['n'] < math.ceil(1 / (1 - claim['level'])) - 1:
                    out.append('I')
            if on('dependence') and c['dependence'] != 'independent':
                out.append('I')
            if on('representativeness'):
                out.extend('I' for s in kinds('shift_diagnostic') if s['value'] > s['threshold'])
            for e in kinds('metric_report', 'outcome_records'):
                if e['metric'] == 'interval_coverage' and e['population'] == claim['population']:
                    before = len(out)
                    bound_rule(rate(e), 'at_least', claim['level'], 0.95)
                    if out[before:] != ['C']:
                        out[before:] = ['O']
    elif t == 'numerical_adequacy':
        if on('applicability') and any(not s['numerical_solver'] for s in kinds('system_description')):
            return 'not_applicable'
        ch = kinds('numerical_checks')
        if not ch:
            out.append('I')
        else:
            checks = ch[0]['checks']
            req = claim['required_checks']
            out.append('C' if any(checks.get(x) is False for x in req) else 'I' if any(checks.get(x) is None for x in req) else 'S')
    elif t == 'provenance':
        items = kinds('file_hash', 'embedded_artifact')
        if not items:
            out.append('I')
        for e in items:
            okv = file_ok(e, root)
            if okv is None:
                out.append('I')
            elif not okv:
                out.append('C')
            else:
                out.append('S')
                if on('authority') and AUTH[e['authority']] < AUTH[claim['authority_required']]:
                    out.append('I')
    elif t in ('no_leakage', 'independence'):
        if t == 'no_leakage' and on('leakage_mechanics'):
            timing, part, cons = kinds('feature_timing'), kinds('partition'), kinds('target_construction')
            if not timing:
                out.append('I')
            else:
                out.append('C' if any(f['available_offset_hours'] > 0 for f in timing[0]['features']) else 'S')
            if not part:
                out.append('I')
            else:
                pt = part[0]
                sp = {s['name']: s for s in pt['splits']}
                bad = any(a_['start'] <= b_['end'] and b_['start'] <= a_['end'] for i, a_ in enumerate(pt['splits']) for b_ in pt['splits'][i + 1:])
                tr, ev_ = sp.get(pt['training_split']), sp.get(pt['evaluation_split'])
                bad = bad or tr is None or ev_ is None or (claim['task'] == 'forecast' and tr['end'] >= ev_['start'])
                bad = bad or (claim['task'] == 'forecast' and pt['ordering'] == 'random')
                bad = bad or any(s != pt['training_split'] for s in pt['preprocessing_fit_splits'])
                out.append('C' if bad else 'S')
            if not cons:
                out.append('I')
            elif not timing:
                out.append('I')
            else:
                names = {f['name'] for f in timing[0]['features']}
                exact = [i for i in cons[0]['identities'] if i['rows'] > 0 and i['max_abs_residual'] == 0 and set(i['features']) <= names]
                out.append('C' if exact else 'S')
        if t == 'independence' and on('dependence'):
            d = kinds('dependence_diagnostic')
            if not d:
                out.append('I')
            else:
                out.append('C' if abs(d[0]['lag1_autocorrelation']) > 1.959963984540054 / math.sqrt(d[0]['n']) else 'S')
        att = kinds('attestation')
        if att:
            out.append('O' if on('attestation_policy') else ('S' if att[0]['value'] else 'C'))
    elif t == 'physical_generalization':
        v = [e for e in kinds('physical_validation') if e['domain'] == claim['to_domain']]
        out.append('I' if not v else 'S' if v[0]['passed'] else 'C')
    elif t == 'universal_reliability':
        rs = rates(pop=False)
        if on('universality'):
            out.append('I')
            for r in rs:
                bb = bounds(r, claim['confidence'], on('dependence'))
                if bb and bb[0] > claim['reliability_bound']:
                    out.append('C')
        else:
            for r in rs:
                bound_rule(r, 'at_most', claim['reliability_bound'], claim['confidence'])
    elif t == 'operational_risk':
        cals = kinds('calibration')
        if not cals:
            out.append('I')
        else:
            count = cals[0]['condition_counts'].get(claim['condition'], 0)
            out.append('S' if (not on('calibration_applicability') or count >= claim['min_condition_support']) else 'I')
            if on('representativeness'):
                sc = kinds('support_check')
                if not sc or sc[0]['value'] > sc[0]['support_max']:
                    out.append('I')
            if on('completeness'):
                cc = kinds('completeness')
                if not cc or cc[0]['available'] < cc[0]['required']:
                    out.append('I')
            if on('statistical'):
                for e in kinds('metric_report', 'outcome_records'):
                    if e['population'] != claim['recent_population']:
                        continue
                    r = rate(e)
                    if r['n'] == 0:
                        continue
                    if on('dependence') and r['dep'] in ('temporal', 'clustered') and r['deff'] is None:
                        dependence(r)
                        continue
                    bb = bounds(r, claim['confidence'], on('dependence'))
                    if bb and bb[0] > claim['bound']:
                        out.append('C')
    if on('limitations'):
        out.extend('I' for e in kinds('limitation') if t in e['affects_claim_types'] and e['effect'] == 'invalidates')
    if integrity_failed:
        return 'insufficient_evidence'
    if 'C' in out:
        return 'contradicted'
    if 'I' in out or 'S' not in out:
        return 'insufficient_evidence'
    return 'supported_within_scope'


def standard_class(a, root, scope):
    claim = a['claims'][0]
    ev = {e['id']: e for e in a['evidence']}
    linked = [ev[i] for i in claim['evidence']]
    out = []
    for e in linked:
        if e['kind'] in ('file_hash', 'embedded_artifact'):
            okv = file_ok(e, root)
            out.append('I' if okv is None else 'S' if okv else 'C')
    kinds = lambda *k: [e for e in linked if e['kind'] in k]  # noqa: E731
    att = kinds('attestation')
    t = claim['type']

    def naive(r):
        if r['k'] is not None and r['k'] > r['n']:
            return None
        if r['k'] is not None and r['n'] > 0:
            return r['k'] / r['n']
        return r['v']

    def bnd(r, cmp, b, g):
        bb = bounds(r, g, False)
        if bb is None:
            out.append('I')
            return
        lo, hi = bb
        sup, con = (hi <= b, lo > b) if cmp == 'at_most' else (lo >= b, hi < b)
        out.append('S' if sup else 'C' if con else 'I')

    metric = [rate(e) for e in kinds('metric_report', 'outcome_records') if e['metric'] == claim.get('metric') and e['population'] == claim.get('population')]
    if t in ('metric_value', 'metric_bound'):
        if not metric:
            out.append('I')
        for r in metric:
            if scope and any(c not in r['cond'] for c in claim['conditions']):
                out.append('I')
            v = naive(r)
            if v is None:
                out.append('C')
                continue
            if t == 'metric_value':
                cv, tol, cmp = claim['claimed_value'], claim['tolerance'], claim['comparator']
                ok = abs(v - cv) <= tol if cmp == 'equals' else v <= cv + tol if cmp == 'at_most' else v >= cv - tol
                out.append('S' if ok else 'C')
            else:
                if claim['claimed_value'] is not None and abs(v - claim['claimed_value']) > claim['tolerance']:
                    out.append('C')
                bnd(r, claim['comparator'], claim['bound'], claim['confidence'])
    elif t == 'coverage_guarantee':
        cals = kinds('calibration')
        if not cals:
            out.append('I')
        else:
            if scope and any(c not in cals[0]['conditions'] for c in claim['conditions']):
                out.append('I')
            out.append('S')
            for e in kinds('metric_report', 'outcome_records'):
                if e['metric'] == 'interval_coverage' and e['population'] == claim['population']:
                    v = naive(rate(e))
                    if v is not None and v < claim['level']:
                        out.append('C')
    elif t == 'numerical_adequacy':
        ch = kinds('numerical_checks')
        if not ch:
            out.append('I')
        else:
            c, req = ch[0]['checks'], claim['required_checks']
            out.append('C' if any(c.get(x) is False for x in req) else 'I' if any(c.get(x) is None for x in req) else 'S')
    elif t == 'provenance':
        if not kinds('file_hash', 'embedded_artifact'):
            out.append('I')
    elif t in ('no_leakage', 'independence'):
        out.append('I' if not att else 'S' if att[0]['value'] else 'C')
    elif t == 'physical_generalization':
        pv = kinds('physical_validation')
        if not pv:
            out.append('I')
        else:
            if scope and pv[0]['domain'] != claim['to_domain']:
                out.append('I')
            out.append('S' if pv[0]['passed'] else 'C')
    elif t == 'universal_reliability':
        rs = [rate(e) for e in kinds('metric_report', 'outcome_records') if e['metric'] == claim['metric']]
        if not rs:
            out.append('I')
        for r in rs:
            bnd(r, 'at_most', claim['reliability_bound'], claim['confidence'])
    elif t == 'operational_risk':
        if not kinds('calibration'):
            out.append('I')
        else:
            out.append('S')
            for e in kinds('metric_report', 'outcome_records'):
                v = naive(rate(e)) if e['population'] == claim['recent_population'] else None
                if v is not None and v > claim['bound']:
                    out.append('C')
    if 'C' in out:
        return 'contradicted'
    if 'I' in out or 'S' not in out:
        return 'insufficient_evidence'
    return 'supported_within_scope'


def wilson(k, n, z=1.959963984540054):
    p = k / n
    c = (p + z * z / (2 * n)) / (1 + z * z / n)
    h = z * math.sqrt(p * (1 - p) / n + z * z / (4 * n * n)) / (1 + z * z / n)
    return [c - h, c + h]


def verify_suite(path, root):
    report = json.loads(Path(path).read_bytes())['report']
    suite = report['suite']
    unsupported = lambda c: c in ('insufficient_evidence', 'contradicted')  # noqa: E731
    preds = {'triad': [], 'standard_checks_c': [], 'standard_checks_c_plus_scope': []}
    for case, got in zip(suite['cases'], report['cases']):
        a = case['assessment']
        t = triad_class(a, root)
        same(got['triad']['classification'], t, case['case_id'] + ' triad')
        same(got['standard']['classification'], standard_class(a, root, False), case['case_id'] + ' C')
        same(got['standard_plus_scope'], standard_class(a, root, True), case['case_id'] + ' C+scope')
        for fam in FAMILIES:
            same(got['ablations'][fam], triad_class(a, root, frozenset([fam])), f"{case['case_id']} ablation {fam}")
        preds['triad'].append(t)
        preds['standard_checks_c'].append(got['standard']['classification'])
        preds['standard_checks_c_plus_scope'].append(got['standard_plus_scope'])
    truth = [c['ground_truth'] for c in suite['cases']]
    for method, p in preds.items():
        s = report['methods'][method]
        uns = [i for i, g in enumerate(truth) if unsupported(g)]
        val = [i for i, g in enumerate(truth) if g == 'supported_within_scope']
        det = sum(unsupported(p[i]) for i in uns)
        same(s['detected'], det, method)
        same(s['false_rejections'], sum(unsupported(p[i]) for i in val), method)
        corr = sum(a == b for a, b in zip(p, truth))
        same(s['four_class_correct'], corr, method)
        close(s['four_class_accuracy_wilson95_suite_internal'], wilson(corr, len(truth)), method + ' wilson')
    t, c = preds['triad'], preds['standard_checks_c']
    b = sum(unsupported(t[i]) and not unsupported(c[i]) for i, g in enumerate(truth) if unsupported(g))
    cc = sum(unsupported(c[i]) and not unsupported(t[i]) for i, g in enumerate(truth) if unsupported(g))
    close(report['primary']['mcnemar_exact_two_sided_suite_internal'],
          stats.binomtest(min(b, cc), b + cc, 0.5).pvalue if b + cc else 1.0, 'mcnemar')
    diff = report['methods']['triad']['detection_rate'] - report['methods']['standard_checks_c']['detection_rate']
    met = diff >= 0.2 - 1e-12 and report['methods']['triad']['false_rejection_rate'] <= report['methods']['standard_checks_c']['false_rejection_rate'] + 0.05 + 1e-12
    same(report['primary']['criterion_met'], met, 'suite primary')
    return report


def verify_frozen():
    manifest = json.loads(Path('benchmarks/bike-sharing/frozen-before-v7.json').read_bytes())
    for name, sha in manifest['sha256'].items():
        same(hashlib.sha256(Path(name).read_bytes()).hexdigest(), sha, name)
    return len(manifest['sha256'])


def plot(bike, plots, suite, directory):
    import matplotlib
    matplotlib.use('Agg')
    import matplotlib.pyplot as plt
    directory.mkdir(parents=True, exist_ok=True)
    colours = {'ridge': '#1f5f8b', 'hgb': '#b5512a'}
    fig, axes = plt.subplots(2, 3, figsize=(16, 9), layout='constrained')
    for name, gen, rep, mon in plots:
        curve = gen['overall']['risk_coverage_curve']
        axes[0, 0].plot([r['coverage'] for r in curve], [r['risk'] for r in curve], color=colours[name], label=name)
        rel = [r for r in gen['overall']['reliability'] if r['count']]
        axes[0, 1].plot([r['mean_probability'] for r in rel], [r['event_rate'] for r in rel], 'o-', color=colours[name], label=name)
        x = [dt.date.fromordinal(d) for d in mon['days']]
        risk = [e / a if a else np.nan for a, e in zip(mon['acc'], mon['err'])]
        axes[1, 0 if name == 'ridge' else 1].plot(x, risk, '-', color=colours[name], lw=1, label=f'{name} daily accepted risk')
        ax = axes[1, 0 if name == 'ridge' else 1]
        ax.axhline(mon['bound'], color='k', ls='--', lw=1, label='degradation bound r_cal + 0.05')
        for pol, mk, off in (('triad', 'v', 1.04), ('std_matched_calibration', '^', 1.10)):
            al = [d for d, a in zip(x, mon['policies'][pol]) if a]
            ax.plot(al, [np.nanmax(risk) * off] * len(al), mk, ms=5, label=f'{pol} alert', color='#444' if pol == 'triad' else '#999')
        ax.set(title=f'{name}: prospective day decisions (test)', ylabel='Accepted material-error fraction')
        ax.legend(fontsize=7)
        ax.tick_params(axis='x', rotation=30)
        months = sorted(rep['conformal']['test_by_month'])
        axes[0, 2].plot(months, [rep['conformal']['test_by_month'][mo]['coverage'] for mo in months], 'o-', color=colours[name], label=name)
    axes[0, 2].axhline(0.9, color='k', ls='--', lw=1, label='nominal 0.90')
    axes[0, 1].plot([0, 1], [0, 1], 'k--', lw=1)
    for ax, title, xl, yl in ((axes[0, 0], 'Retrospective risk-coverage (test)', 'Coverage', 'Material-error fraction'),
                              (axes[0, 1], 'Error-probability reliability (test)', 'Mean predicted probability', 'Observed frequency'),
                              (axes[0, 2], 'Normalised conformal coverage by month', 'Month', 'Coverage')):
        ax.set(title=title, xlabel=xl, ylabel=yl)
        ax.legend(fontsize=8)
        ax.grid(alpha=.2)
    ax = axes[1, 2]
    for k, (name, _, rep, _) in enumerate(plots):
        for j, blk in enumerate(rep['monitoring']['primary']['comparison']['bootstrap']):
            iv = blk['intervals'][0]['interval']
            d = rep['monitoring']['primary']['comparison']['delta_mean_cost_a_minus_b']
            ax.errorbar(k * 4 + j, d, yerr=[[d - iv[0]], [iv[1] - d]], fmt='o', color=colours[name], capsize=3)
            ax.annotate(f"{blk['block_days']}d", (k * 4 + j, iv[1]), fontsize=7, ha='center', va='bottom')
    ax.axhline(0, color='k', lw=1)
    for v in (0.01, -0.01):
        ax.axhline(v, color='grey', ls=':', lw=1)
    ax.set_xticks([1, 5], ['ridge', 'hgb'])
    ax.set(title='Primary: cost(TRIAD) - cost(std, matched budget)\n97.5% paired block intervals; dotted = practical threshold', ylabel='Mean cost per eligible hour')
    fig.suptitle('UCI Bike Sharing (2012 test period) - frozen models - exploratory; hours and days are dependent, not independent trials', fontsize=12)
    fig.savefig(directory / 'bike-sharing.png', dpi=160)
    fig.savefig(directory / 'bike-sharing.pdf')
    plt.close(fig)
    fig, ax = plt.subplots(figsize=(10, 4.5), layout='constrained')
    cats = sorted(set(c['category'] for c in suite['cases']))
    width = 0.27
    for j, (method, key) in enumerate((('TRIAD', 'triad'), ('Standard checks C', 'standard_checks_c'))):
        rates = []
        for cat in cats:
            s = suite['by_category'][key][cat]
            rates.append(s['detection_rate'] if s['detection_rate'] is not None else np.nan)
        ax.bar(np.arange(len(cats)) + (j - 0.5) * width, rates, width, label=method)
    ax.set_xticks(range(len(cats)), cats, rotation=35, ha='right')
    ax.set(ylabel='Unsupported claims detected', title='Claim-validity suite by category (author-constructed; valid_claim has no unsupported cases)')
    ax.legend()
    fig.savefig(directory / 'evidence-suite.png', dpi=160)
    fig.savefig(directory / 'evidence-suite.pdf')
    plt.close(fig)


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--bike', type=Path, required=True)
    ap.add_argument('--suite', type=Path, required=True)
    ap.add_argument('--evidence', type=Path)
    ap.add_argument('--data', type=Path)
    ap.add_argument('--plots', type=Path)
    ap.add_argument('--root', type=Path, default=Path('.'))
    args = ap.parse_args()
    bike, plots = verify_bike(args.bike, args.data)
    suite = verify_suite(args.suite, args.root)
    if args.evidence:
        rep = json.loads(args.evidence.read_bytes())['report']
        for f in rep['findings']:
            claim = next(c for c in rep['input']['claims'] if c['id'] == f['claim_id'])
            single = dict(rep['input'], claims=[claim])
            same(f['classification'], triad_class(single, args.root), 'example ' + f['claim_id'])
    frozen = verify_frozen()
    if args.plots:
        plot(bike, plots, suite, args.plots)
    print(f'PASS: {CHECKS} independent checks (bike benchmark, conformal, monitoring policies, bootstrap intervals, '
          f'claim-suite classifications and ablations) and {frozen} frozen pre-M7 artifacts.')


if __name__ == '__main__':
    main()
