"""Independent standard-library metric check. Usage: python scripts/verify-metrology.py REPORT."""
import json
import math
import sys
from pathlib import Path

envelope = json.loads(Path(sys.argv[1]).read_text())
r = envelope['report']
data = r['input']
cal = sorted(x['uncertainty'] for x in data['records'] if x['split'] == 'calibration' and x['scenario'] == data['calibration_scenario'])
k = math.ceil(len(cal) * data['target_coverage'])
threshold = cal[k-1] if k else None
assert threshold == r['threshold']
rows = [x for x in data['records'] if x['split'] == 'test']

def event(x):
    if x['truth'] is None:
        return None
    loss = float(x['prediction'] != x['truth']) if data['loss'] == 'classification' else math.dist(x['prediction'], x['truth'])
    return loss > data['tolerance']

def close(actual, expected):
    if expected is None:
        assert actual is None
    else:
        assert math.isclose(actual, expected, rel_tol=1e-12, abs_tol=1e-12), (actual, expected)

def check(rows, report):
    accepted = [x for x in rows if threshold is not None and x['uncertainty'] <= threshold]
    events = [event(x) for x in rows]
    ae = [event(x) for x in accepted]
    m = report['operational']
    assert m['count'] == len(rows) and m['accepted'] == len(accepted)
    close(m['coverage'], len(accepted)/len(rows))
    close(m['selective_error_risk'], sum(ae)/len(ae) if ae and None not in ae else None)
    close(m['incorrect_acceptance_frequency'], sum(ae)/len(rows) if None not in events else None)
    ordered = sorted(rows, key=lambda x: (x['uncertainty'], x['id']))
    area = 0.0
    previous = 0
    for i, x in enumerate(ordered):
        if i+1 == len(ordered) or x['uncertainty'] != ordered[i+1]['uncertainty']:
            if None not in events:
                area += (i+1-previous)/len(rows)*sum(event(y) for y in ordered[:i+1])/(i+1)
            previous = i+1
    close(report['aurc'], area if None not in events else None)
    if all(x['error_probability'] is not None for x in rows) and None not in events:
        close(report['brier'], sum((x['error_probability']-event(x))**2 for x in rows)/len(rows))
        bins = [[] for _ in range(data['reliability_bins'])]
        for x in rows:
            bins[min(int(x['error_probability']*len(bins)), len(bins)-1)].append(x)
        ece = sum(abs(sum(x['error_probability']-event(x) for x in b)) for b in bins)/len(rows)
        close(report['ece'], ece)

check(rows, r['overall'])
for name, report in r['scenarios'].items():
    check([x for x in rows if x['scenario'] == name], report)
print('Independent coverage, risk, incorrect acceptance, AURC, ECE and Brier checks passed.')
