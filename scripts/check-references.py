"""Machine-check the Milestone 7 reference registry (docs/metrology/milestone7-references.json).

Usage: python scripts/check-references.py [--record NEW_FILE.json]
Requires network access. For each DOI: the DOI handle must resolve and, when
Crossref or DataCite returns a title, the expected title words must appear in it.
For each URL: the page must load (HTTP 200) and contain the expected words in
its title (PDFs: loadable only). Also fails if a Milestone 7 document cites a
DOI absent from the registry. A passing check establishes existence and title
agreement only, not that a source supports the statement attributed to it.
"""
import argparse
import json
import re
import urllib.parse
import urllib.request
from pathlib import Path

DOCS = ['docs/metrology/evidence-sufficiency-method.md', 'docs/metrology/milestone7-literature-review.md',
        'docs/metrology/milestone7-results.md', 'docs/metrology/milestone7-manuscript.md',
        'docs/metrology/milestone7-research-audit.md', 'docs/metrology/nist-readiness.md']


def get(url, accept='application/json'):
    req = urllib.request.Request(url, headers={'Accept': accept, 'User-Agent': 'triad-reference-check (mailto:ciao_chris@proton.me)'})
    with urllib.request.urlopen(req, timeout=60) as r:
        return r.status, r.read()


def words(text):
    return re.sub(r'[^a-z0-9+\- ]', ' ', text.lower())


def check(ref):
    result = {'key': ref['key']}
    if ref['doi']:
        q = urllib.parse.quote(ref['doi'], safe='/')
        _, body = get(f'https://doi.org/api/handles/{q}')
        result['doi_resolves'] = json.loads(body).get('responseCode') == 1
        title = None
        for api, path in ((f'https://api.crossref.org/works/{q}', ('message', 'title', 0)),
                          (f'https://api.datacite.org/dois/{q}', ('data', 'attributes', 'titles', 0, 'title'))):
            try:
                value = json.loads(get(api)[1])
                for p in path:
                    value = value[p]
                title = value
                break
            except Exception:
                continue
        result['registered_title'] = title
        ok = result['doi_resolves'] and (title is None or not ref['expected_title_words']
                                         or all(w in words(title) for w in ref['expected_title_words']))
    elif ref['url']:
        status, body = get(ref['url'], accept='text/html,application/pdf')
        text = body.decode('utf-8', 'ignore')
        match = re.search(r'<title>(.*?)</title>', text, re.S)
        result['page_title'] = match.group(1).strip() if match else ('PDF' if body[:4] == b'%PDF' else None)
        ok = status == 200 and (not ref['expected_title_words'] or body[:4] == b'%PDF'
                                or all(w in words(result['page_title'] or '') for w in ref['expected_title_words']))
    else:
        result['machine_verifiable'] = False
        ok = True
    result['ok'] = ok
    return result


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--record', type=Path)
    args = ap.parse_args()
    registry = json.loads(Path('docs/metrology/milestone7-references.json').read_text(encoding='utf-8'))['references']
    known = {r['doi'].lower() for r in registry if r['doi']}
    cited = set()
    for doc in DOCS:
        for doi in re.findall(r'(?:doi\.org/|doi:\s?)(10\.[0-9]+/[^\s)\]>,]+)', Path(doc).read_text(encoding='utf-8')):
            cited.add(doi.rstrip('.').lower())
    missing = sorted(d for d in cited if d not in known)
    results = [check(r) for r in registry]
    failures = [r for r in results if not r['ok']]
    record = {'references': len(results), 'failures': failures, 'dois_cited_but_unregistered': missing,
              'not_machine_verifiable': [r['key'] for r in results if r.get('machine_verifiable') is False], 'results': results}
    if args.record:
        with args.record.open('x', encoding='utf-8', newline='\n') as f:
            json.dump(record, f, indent=1)
            f.write('\n')
    if failures or missing:
        raise SystemExit(f'FAIL: {len(failures)} unresolved references; unregistered DOIs: {missing}')
    print(f"PASS: {len(results)} references checked; not machine-verifiable: {record['not_machine_verifiable']}")


if __name__ == '__main__':
    main()
