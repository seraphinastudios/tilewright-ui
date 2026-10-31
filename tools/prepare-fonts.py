#!/usr/bin/env python3
"""Prepare the exact OFL font pack outside this repository; never fetch implicitly."""
# Copyright 2026 Seraphina Studios. SPDX-License-Identifier: MIT
import argparse
import json
from pathlib import Path
import sys

sys.dont_write_bytecode = True

from font_pack import MAX_MANIFEST, acquire, copy_pack, read_regular


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("destination", type=Path, help="Absent directory outside the source checkout")
    source = parser.add_mutually_exclusive_group(required=True)
    source.add_argument("--acquire", action="store_true", help="Explicitly download exact public upstream bytes over HTTPS")
    source.add_argument("--source", type=Path, help="Copy and verify an existing unpacked font pack")
    args = parser.parse_args()
    repository = Path(__file__).resolve().parents[1]
    destination = args.destination.absolute()
    if destination.resolve().is_relative_to(repository):
        parser.error("font payload must be prepared outside the source checkout")
    manifest = read_regular(repository / "tools/unicode-font-pack.json", MAX_MANIFEST)
    if args.acquire:
        record = acquire(destination, manifest)
    else:
        record = copy_pack(args.source.absolute(), destination, manifest)
    print(json.dumps(record, indent=2))


if __name__ == "__main__":
    main()
