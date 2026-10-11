#!/usr/bin/env python3
"""Platform-independent TRIAD CLI output smoke checks."""
import json
import sys
from pathlib import Path

def main():
    if len(sys.argv) != 3:
        raise SystemExit("usage: verify-portable.py ABSTENTION.json CAPTURE.json")
    policy = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
    capture = json.loads(Path(sys.argv[2]).read_text(encoding="utf-8"))
    assert policy["status"] == "refused", "small calibration fixture should refuse certification"
    assert policy["threshold"] is None, "refused policy must not expose threshold"
    assert capture["empty_reference_frames"] == 4
    assert capture["space"] == "synthetic2d"
    print("PASS: portable abstention/capture contract checks")

if __name__ == "__main__":
    main()
