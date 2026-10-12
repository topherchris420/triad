#!/usr/bin/env python3
"""Confirm every URL in the NIST candidate answers 200 without redirecting.

CONTENT_STANDARDS.md requires that no link is broken, moved or redirecting. This
script requests each URL once with redirects disabled and records the result.
It needs network access, so it is a pre-submission gate, not a CI step.

  python scripts/check-nist-links.py [--output RESULT.json]

Exit status 0 only if every link returns HTTP 200 directly.
"""
import argparse
import datetime as dt
import json
import re
import sys
import urllib.error
import urllib.request
from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parents[1]
CANDIDATE = ROOT / "docs/metrology/nist-submission/submissions/selective-risk-at-target-coverage.yml"
URL = re.compile(r"https?://[^\s\"')]+")


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *args, **kwargs):
        return None


def status(url):
    opener = urllib.request.build_opener(NoRedirect)
    request = urllib.request.Request(url, headers={"User-Agent": "triad-link-check/1"})
    try:
        with opener.open(request, timeout=30) as response:
            return response.status, None
    except urllib.error.HTTPError as exc:
        return exc.code, exc.headers.get("Location")
    except (urllib.error.URLError, TimeoutError) as exc:
        return None, str(exc)


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    candidate = yaml.safe_load(CANDIDATE.read_text(encoding="utf-8"))
    urls = []
    for key in ("references", "implementation_resources"):
        for entry in candidate[key]:
            urls += URL.findall(entry)
    results = []
    for url in dict.fromkeys(urls):
        code, note = status(url)
        results.append({"url": url, "status": code, "ok": code == 200, "note": note})
        print(f"{'OK ' if code == 200 else 'BAD'} {code} {url}" + (f"  ({note})" if note else ""))
    if args.output:
        args.output.write_text(json.dumps({
            "checked_on": dt.date.today().isoformat(),
            "candidate": CANDIDATE.relative_to(ROOT).as_posix(),
            "redirects_followed": False,
            "results": results,
            "all_ok": all(r["ok"] for r in results),
        }, indent=1) + "\n", encoding="utf-8")
    return 0 if all(r["ok"] for r in results) else 1


if __name__ == "__main__":
    sys.exit(main())
