"""Inventory NIST SD19 digit sessions without reading image or label payloads."""

import argparse
from collections import Counter, defaultdict
import hashlib
import json
from pathlib import Path
import re
from zipfile import ZipFile

SCHEMA = "triad.sd19-writer-inventory/v1"
DEVELOPMENT = {0, 1}
TARGET = {2, 3, 6, 7, 8}
SHIFT = 4
SALT = "triad-sd19-writer-split-v1"
MIN_DIGITS = 32
MIN_TARGET_WRITERS = 800  # An engineering floor, not a power calculation.
# Accept an optional archive wrapper; match ID and form suffix across all three fields.
DIGIT = re.compile(
    r"(?:^|/)hsf_(?P<part>[0-9]+)/f(?P<id>[0-9]{4})_(?P<form>[0-9]{2})/"
    r"d(?P=id)_(?P=form)/d(?P=id)_(?P=form)_[0-9]+\.png$"
)


def role(part, writer):
    if part in DEVELOPMENT:
        return "development"
    if part == SHIFT:
        return "shift"
    digest = hashlib.sha256(f"{SALT}|{writer}".encode("ascii")).digest()
    return "calibration" if digest[0] < 128 else "audit"


def inventory(archive):
    counts = Counter()
    partition_of = {}
    paths = set()
    for member in archive.infolist():
        match = DIGIT.search(member.filename)
        if not match:
            continue
        part, writer = int(match["part"]), match["id"]
        if part not in DEVELOPMENT | TARGET | {SHIFT}:
            raise ValueError(f"unexpected digit partition: hsf_{part}")
        if member.filename in paths:
            raise ValueError("duplicate digit member in ZIP")
        paths.add(member.filename)
        if partition_of.setdefault(writer, part) != part:
            raise ValueError("the same writer ID appears in multiple partitions")
        counts[writer] += 1
    if not counts:
        raise ValueError("no SD19 by_write digit members found")

    per_part = defaultdict(Counter)
    per_role = Counter()
    for writer, n in counts.items():
        part = partition_of[writer]
        per_part[part]["writer_ids"] += 1
        if n >= MIN_DIGITS:
            per_part[part]["eligible_writer_ids"] += 1
            per_role[role(part, writer)] += 1

    roles = ("development", "calibration", "audit", "shift")
    observed = {r: per_role[r] for r in roles}
    return {
        "schema": SCHEMA,
        "archive_members_only": True,
        "digit_member_manifest_sha256": hashlib.sha256("\n".join(sorted(paths)).encode("utf-8")).hexdigest(),
        "split_salt": SALT,
        "minimum_digit_members_per_writer": MIN_DIGITS,
        "minimum_writers_per_target_split": MIN_TARGET_WRITERS,
        "partitions": {
            f"hsf_{i}": {key: per_part[i][key] for key in ("writer_ids", "eligible_writer_ids")}
            for i in sorted(DEVELOPMENT | TARGET | {SHIFT})
        },
        "eligible_writers_by_role": observed,
        "inventory_floor_met": (
            observed["development"] > 0
            and observed["shift"] > 0
            and min(observed["calibration"], observed["audit"]) >= MIN_TARGET_WRITERS
        ),
        "not_verified": ["image validity", "label linkage", "independence", "power", "certification"],
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("archive", type=Path, help="local NIST SD19 second-edition by_write.zip")
    args = parser.parse_args()
    with ZipFile(args.archive) as archive:
        report = inventory(archive)
    print(json.dumps(report, indent=2))
    return 0 if report["inventory_floor_met"] else 2


if __name__ == "__main__":
    raise SystemExit(main())
