"""Pre-fitting audit of UCI Bike Sharing (dataset 275), hour.csv.

Usage: python scripts/audit-bike-sharing.py --cache DIRECTORY --output NEW_FILE.json

Standard library only. It verifies the archive and file hashes, parses local
wall-clock timestamps, maps them to elapsed UTC hours with the IANA
America/New_York rules, and reports ordering, duplicates, missing elapsed hours,
DST artefacts and calendar consistency for ALL rows. Target values (cnt, casual,
registered) are read ONLY for the 2011 training period. No 2012 target value is
inspected, summarised or written.
"""
import argparse
import collections
import csv
import datetime as dt
import hashlib
import io
import json
import math
import statistics
import urllib.request
import zipfile
from pathlib import Path
from zoneinfo import ZoneInfo

ARCHIVE_URL = 'https://archive.ics.uci.edu/static/public/275/bike+sharing+dataset.zip'
METADATA_URL = 'https://archive.ics.uci.edu/api/dataset?id=275'
ARCHIVE_SHA256 = 'b70182d0d0508e9abbb79306ce5c0cec34869000f8220175ac83d11dbe845401'
HOUR_SHA256 = 'e03de4ee4ef4dc376ac6e04bf829673c6269e8eba5c60fa121640fa2f829504f'
NEW_YORK = ZoneInfo('America/New_York')
EPOCH = dt.datetime(2011, 1, 1, 5, tzinfo=dt.timezone.utc)  # 2011-01-01 00:00 EST
TRAINING_END = '2012-01-01'


def fetch(cache):
    cache.mkdir(parents=True, exist_ok=True)
    for name, url in (('bike-sharing.zip', ARCHIVE_URL), ('uci-metadata.json', METADATA_URL)):
        path = cache / name
        if not path.exists():
            with urllib.request.urlopen(url, timeout=120) as response:
                path.write_bytes(response.read())
    archive = (cache / 'bike-sharing.zip').read_bytes()
    with zipfile.ZipFile(io.BytesIO(archive)) as z:
        members = {n: z.read(n) for n in z.namelist()}
    return archive, members, (cache / 'uci-metadata.json').read_bytes()


def elapsed_hour(day, hour):
    """Elapsed hours since 2011-01-01 00:00 EST. Ambiguous fall-back labels map to
    the first (EDT) occurrence; non-existent spring-forward labels are rejected."""
    local = dt.datetime.fromisoformat(day).replace(hour=hour, tzinfo=NEW_YORK)
    roundtrip = local.astimezone(dt.timezone.utc).astimezone(NEW_YORK)
    if roundtrip.replace(tzinfo=None) != local.replace(tzinfo=None):
        raise ValueError(f'non-existent local time {day} {hour:02d}:00')
    delta = local.astimezone(dt.timezone.utc) - EPOCH
    return int(delta.total_seconds() // 3600)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--cache', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    archive, members, metadata_bytes = fetch(args.cache)
    archive_sha = hashlib.sha256(archive).hexdigest()
    hour_bytes = members['hour.csv']
    assert hashlib.sha256(hour_bytes).hexdigest() == HOUR_SHA256, 'hour.csv changed; do not substitute data'
    metadata = json.loads(metadata_bytes)['data']
    rows = list(csv.DictReader(io.StringIO(hour_bytes.decode('ascii'))))
    expected_columns = ['instant', 'dteday', 'season', 'yr', 'mnth', 'hr', 'holiday', 'weekday',
                        'workingday', 'weathersit', 'temp', 'atemp', 'hum', 'windspeed',
                        'casual', 'registered', 'cnt']
    assert list(rows[0].keys()) == expected_columns
    local = [(r['dteday'], int(r['hr'])) for r in rows]
    elapsed = [elapsed_hour(d, h) for d, h in local]
    present = set(elapsed)
    span = range(elapsed[0], elapsed[-1] + 1)
    missing = [t for t in span if t not in present]
    runs = []
    for t in missing:
        if runs and t == runs[-1][-1] + 1:
            runs[-1].append(t)
        else:
            runs.append([t])

    def label(t):
        moment = (EPOCH + dt.timedelta(hours=t)).astimezone(NEW_YORK)
        return moment.strftime('%Y-%m-%dT%H:00%z')

    labels_missing = sorted({(d.isoformat(), h) for d in
                             (dt.date(2011, 1, 1) + dt.timedelta(days=i) for i in range(731))
                             for h in range(24)} - {(d, h) for d, h in local})
    nonexistent = []
    for d, h in labels_missing:
        try:
            elapsed_hour(d, h)
        except ValueError:
            nonexistent.append(f'{d}T{h:02d}')
    calendar_errors = collections.Counter()
    for r, (d, h) in zip(rows, local):
        day = dt.date.fromisoformat(d)
        calendar_errors['weekday'] += int(r['weekday']) != (day.weekday() + 1) % 7
        calendar_errors['yr_mnth'] += int(r['yr']) != day.year - 2011 or int(r['mnth']) != day.month
        calendar_errors['workingday'] += int(r['workingday']) != int(day.weekday() < 5 and r['holiday'] == '0')
        calendar_errors['hour_range'] += not 0 <= h <= 23
    holidays = sorted({d for (d, _), r in zip(local, rows) if r['holiday'] == '1'})
    training = [r for r in rows if r['dteday'] < TRAINING_END]
    cnt = [int(r['cnt']) for r in training]
    assert all(int(r['casual']) + int(r['registered']) == int(r['cnt']) for r in training)
    by_month = collections.defaultdict(list)
    profile = collections.defaultdict(list)
    log_by_time = {}
    for r, c in zip(training, cnt):
        by_month[int(r['mnth'])].append(c)
        profile[(r['workingday'], int(r['hr']))].append(c)
        log_by_time[elapsed_hour(r['dteday'], int(r['hr']))] = math.log1p(c)

    def autocorrelation(lag):
        pairs = [(v, log_by_time[t - lag]) for t, v in log_by_time.items() if t - lag in log_by_time]
        a, b = zip(*pairs)
        ma, mb = statistics.fmean(a), statistics.fmean(b)
        num = sum((x - ma) * (y - mb) for x, y in pairs)
        den = math.sqrt(sum((x - ma) ** 2 for x in a) * sum((y - mb) ** 2 for y in b))
        return {'lag_hours': lag, 'pairs': len(pairs), 'pearson_log1p': round(num / den, 4)}

    ordered = sorted(cnt)
    report = {
        'status': 'pre-fitting audit; 2012 targets not read',
        'source': {
            'uci_id': 275, 'doi': metadata['dataset_doi'], 'repository_url': metadata['repository_url'],
            'archive_url': ARCHIVE_URL, 'archive_sha256_observed': archive_sha,
            'archive_sha256_recorded_2026_10_11': ARCHIVE_SHA256,
            'archive_matches_recorded': archive_sha == ARCHIVE_SHA256,
            'member_sha256': {n: hashlib.sha256(b).hexdigest() for n, b in sorted(members.items())},
            'license': 'CC BY 4.0 (stated on the UCI dataset page; the API response has no licence field)',
            'citation': 'Fanaee-T, H. (2013). Bike Sharing [Dataset]. UCI Machine Learning Repository. https://doi.org/10.24432/C5W894',
            'metadata_num_instances': metadata['num_instances'],
            'metadata_has_missing_values': metadata['has_missing_values'],
            'metadata_season_coding': next(v['description'] for v in metadata['variables'] if v['name'] == 'season'),
        },
        'rows': len(rows),
        'discrepancies': [
            f"UCI metadata lists {metadata['num_instances']} instances; hour.csv has {len(rows)} rows (Readme.txt states 17379).",
            "Readme.txt codes season as 1 spring..4 winter; the UCI page codes 1 winter..4 fall. January rows carry season 1, matching the UCI page. Season is not used as a predictor.",
            "'No missing values' refers to cells. Elapsed-hour gaps exist and are reported below.",
        ],
        'chronology': {
            'timestamp_semantics': 'dteday + hr are local wall-clock labels (America/New_York); the 02:00 label is absent on both spring-forward days',
            'instant_consecutive': [int(r['instant']) for r in rows] == list(range(1, len(rows) + 1)),
            'strictly_increasing_elapsed': all(b > a for a, b in zip(elapsed, elapsed[1:])),
            'duplicate_local_labels': len(local) - len(set(local)),
            'duplicate_elapsed_hours': len(elapsed) - len(present),
            'first_local': f'{local[0][0]}T{local[0][1]:02d}', 'last_local': f'{local[-1][0]}T{local[-1][1]:02d}',
            'elapsed_span_hours': len(span),
            'missing_elapsed_hours': len(missing),
            'missing_runs': len(runs),
            'missing_run_length_counts': dict(sorted(collections.Counter(len(r) for r in runs).items())),
            'missing_runs_of_6_or_more': [{'first': label(r[0]), 'last': label(r[-1]), 'hours': len(r)} for r in runs if len(r) >= 6],
            'missing_by_local_hour': dict(sorted(collections.Counter(
                (EPOCH + dt.timedelta(hours=t)).astimezone(NEW_YORK).hour for t in missing).items())),
            'missing_local_labels': len(labels_missing),
            'nonexistent_local_labels_spring_forward': nonexistent,
            'fall_back_note': 'Fall-back days have 24 labels; the repeated 01:00 hour cannot be separated, so it is mapped to the first (EDT) occurrence and the second appears as one missing elapsed hour.',
            'interpretation': 'An absent hour may be zero rentals, an outage or a recording gap; it is unknown, never imputed as zero.',
        },
        'calendar_consistency_errors': dict(calendar_errors),
        'holidays': holidays,
        'training_2011_targets_only': {
            'rows': len(training), 'cnt_equals_casual_plus_registered': True,
            'mean': round(statistics.fmean(cnt), 3), 'median': statistics.median(cnt),
            'p90': ordered[int(0.9 * len(ordered))], 'p99': ordered[int(0.99 * len(ordered))],
            'max': max(cnt), 'min': min(cnt),
            'monthly_mean': {m: round(statistics.fmean(v), 1) for m, v in sorted(by_month.items())},
            'mean_profile_working_day': [round(statistics.fmean(profile[('1', h)])) for h in range(24)],
            'mean_profile_non_working_day': [round(statistics.fmean(profile[('0', h)])) for h in range(24)],
            'autocorrelation': [autocorrelation(l) for l in (1, 2, 24, 168)],
        },
    }
    with args.output.open('x', encoding='utf-8', newline='\n') as f:
        json.dump(report, f, indent=1)
        f.write('\n')
    print(f'Audit written; archive {"matches" if report["source"]["archive_matches_recorded"] else "DIFFERS from"} recorded bytes; hour.csv verified.')


if __name__ == '__main__':
    main()
