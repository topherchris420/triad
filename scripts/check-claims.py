#!/usr/bin/env python3
"""Bind the README's headline numbers to the archived evidence (docs/claims-ledger.json).

  python scripts/check-claims.py            verify; exit 1 on any drift
  python scripts/check-claims.py --write    re-render the README findings table

For every claim in the ledger this script

1. reads the frozen artifact and resolves the claim (a JSON pointer, or a count
   over a list of records), applies its declared transform and rounding, and
   requires the result to equal the number displayed in the ledger;
2. requires the README to contain the claim's quoted sentence verbatim, and the
   quote to contain the displayed number;
3. requires the README block between the claims-ledger markers to equal the
   findings table rendered from the ledger.

Standard library only. Read-only unless --write. This verifies transcription
from evidence to prose. It does not verify that the evidence is scientifically
valid: that is what the protocols, verifiers and negative results are for.
"""
import argparse
import json
import operator
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
LEDGER = ROOT / "docs/claims-ledger.json"
README = ROOT / "README.md"
START = "<!-- claims-ledger:start (rendered from docs/claims-ledger.json by scripts/check-claims.py --write; edit the ledger, not this table) -->"
END = "<!-- claims-ledger:end -->"
OPS = {"==": operator.eq, "!=": operator.ne, ">": operator.gt, ">=": operator.ge, "<": operator.lt, "<=": operator.le}
MINUS = "−"


def resolve(doc, pointer):
    """RFC 6901 JSON pointer."""
    node = doc
    for raw in pointer.split("/")[1:]:
        key = raw.replace("~1", "/").replace("~0", "~")
        node = node[int(key)] if isinstance(node, list) else node[key]
    return node


def evaluate(claim, cache):
    path = claim["artifact"]
    if path not in cache:
        cache[path] = json.loads((ROOT / path).read_bytes())
    doc = cache[path]
    if "count_where" in claim:
        conds = [(field, OPS[op], value) for field, op, value in claim["count_where"]]
        return sum(all(op(rec[field], value) for field, op, value in conds) for rec in doc)
    return resolve(doc, claim["pointer"])


def display(value, claim):
    """Format a value exactly as the README prints it."""
    if "expect" in claim:
        if value != claim["expect"]:
            raise ValueError(f"artifact holds {value!r}, ledger expects {claim['expect']!r}")
        return claim["display"]
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise ValueError(f"non-numeric value {value!r} needs an 'expect' field")
    transform = claim.get("transform")
    if transform == "percent":
        value = value * 100
    elif transform == "complement_percent":
        value = (1 - value) * 100
    elif transform is not None:
        raise ValueError(f"unknown transform {transform!r}")
    decimals = claim.get("decimals")
    if decimals is None:
        if value != int(value):
            raise ValueError(f"{value!r} is not an integer; declare decimals")
        value = int(value)
    sep = "," if claim.get("thousands") else ""
    text = f"{abs(value):{sep}}" if decimals is None else f"{abs(value):{sep}.{decimals}f}"
    if claim.get("signed"):
        text = ("+" if round(value, decimals or 0) >= 0 else MINUS) + text
    elif value < 0:
        text = MINUS + text
    return text + claim.get("suffix", "")


def render(ledger, shown):
    rows = [
        "| Milestone | Question | What the archived evidence says | Status | Source |",
        "|:--:|---|---|---|---|",
    ]
    for f in ledger["findings"]:
        cells = [f["milestone"], f["question"], f["finding"].format_map(shown), f["status"].format_map(shown),
                 f"[{Path(f['source']).name}]({f['source']})"]
        rows.append("| " + " | ".join(cells) + " |")
    return "\n".join(rows)


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--write", action="store_true", help="re-render the README findings table in place")
    args = ap.parse_args()
    for stream in (sys.stdout, sys.stderr):  # quotes contain Δ and −; a cp1252 console cannot print them
        stream.reconfigure(encoding="utf-8", errors="backslashreplace")
    ledger = json.loads(LEDGER.read_text(encoding="utf-8"))
    readme = README.read_text(encoding="utf-8")
    failures, shown, cache = [], {}, {}

    for cid, claim in ledger["claims"].items():
        try:
            shown[cid] = display(evaluate(claim, cache), claim)
        except (KeyError, IndexError, TypeError, ValueError, OSError) as exc:
            failures.append(f"{cid}: cannot evaluate {claim['artifact']}: {exc}")
            continue
        if shown[cid] != claim["display"]:
            failures.append(f"{cid}: artifact gives {shown[cid]!r}, ledger displays {claim['display']!r}")
        quote = claim.get("quote")
        if quote is not None:
            if claim["display"] not in quote:
                failures.append(f"{cid}: quote does not contain {claim['display']!r}")
            if quote not in readme:
                failures.append(f"{cid}: README no longer contains {quote!r}")

    for f in ledger["findings"]:
        if not (ROOT / f["source"]).is_file():
            failures.append(f"finding {f['question']!r}: missing source {f['source']}")
    if failures:
        print("\n".join(["FAIL"] + failures), file=sys.stderr)
        return 1

    table = render(ledger, shown)
    if START not in readme or END not in readme:
        print(f"FAIL\nREADME is missing the claims-ledger markers:\n{START}\n{END}", file=sys.stderr)
        return 1
    head, rest = readme.split(START, 1)
    current, tail = rest.split(END, 1)
    if current.strip("\n") != table:
        if not args.write:
            print("FAIL\nREADME findings table differs from the ledger; run: python scripts/check-claims.py --write",
                  file=sys.stderr)
            return 1
        README.write_text(f"{head}{START}\n{table}\n{END}{tail}", encoding="utf-8", newline="\n")
        print("README findings table re-rendered")
    quoted = sum("quote" in c for c in ledger["claims"].values())
    print(f"PASS: {len(shown)} claims recomputed from {len(cache)} artifacts; "
          f"{quoted} README quotes and {len(ledger['findings'])} findings rows match")
    return 0


if __name__ == "__main__":
    sys.exit(main())
