#!/usr/bin/env python3
"""Check TRIAD's NIST *candidate* against the vendored official NIST schema v1.0.

This is a mechanical schema validator, never a scientific validation or submission.
Run from any working directory. Requires PyYAML and jsonschema.
"""
import argparse
import json
import sys
from pathlib import Path

import yaml
from jsonschema import Draft202012Validator

ROOT = Path(__file__).resolve().parents[1]
SCHEMA = ROOT / "docs/metrology/nist-schema-v1.json"
DRAFT = ROOT / "docs/metrology/nist-submission/submissions/selective-risk-at-target-coverage.yml"


def errors_for(candidate, schema):
    return sorted(Draft202012Validator(schema).iter_errors(candidate), key=lambda e: str(list(e.path)))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--self-test", action="store_true", help="Assert rejection of malformed candidate submissions")
    args = parser.parse_args()
    try:
        schema = json.loads(SCHEMA.read_text(encoding="utf-8"))
        candidate = yaml.safe_load(DRAFT.read_text(encoding="utf-8"))
        Draft202012Validator.check_schema(schema)
        errors = errors_for(candidate, schema)
        if errors:
            for issue in errors:
                print(f"INVALID at {'.'.join(map(str, issue.path)) or '<root>'}: {issue.message}", file=sys.stderr)
            return 1
        if args.self_test:
            variants = []
            missing = dict(candidate)
            missing.pop("contact_email", None)
            variants.append(missing)
            placeholder = dict(candidate)
            placeholder["contact_email"] = "<PUBLIC PROFESSIONAL CONTACT REQUIRED>"
            variants.append(placeholder)
            extra_field = dict(candidate)
            extra_field["nist_endorsed"] = True
            variants.append(extra_field)
            duplicate = dict(candidate)
            duplicate["references"] = [candidate["references"][0]] * 2
            variants.append(duplicate)
            if any(not errors_for(item, schema) for item in variants):
                print("Self-test failed: NIST schema accepted an invalid candidate", file=sys.stderr)
                return 2
            print("PASS: four intentionally invalid candidate submissions rejected")
        else:
            print("PASS: local candidate conforms to vendored NIST submission schema v1.0")
            print("NOTE: format validity does not imply scientific readiness, NIST review or submission")
        return 0
    except (OSError, json.JSONDecodeError, yaml.YAMLError, TypeError, ValueError) as exc:
        print(f"Cannot validate NIST draft: {exc}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
