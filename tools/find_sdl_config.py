#!/usr/bin/env python3
"""Select only the SDL prefix emitted by this Cargo invocation."""
import argparse
import json
from pathlib import Path
import re


def select(report, target):
    target = target.resolve(strict=True)
    outputs = set()
    for line in report.splitlines():
        item = json.loads(line)
        if item.get("reason") == "build-script-executed" and "#sdl3-sys@" in item.get("package_id", ""):
            outputs.add(Path(item["out_dir"]).resolve(strict=True))
    if len(outputs) != 1:
        raise ValueError("expected exactly one SDL3 build-script output from the current Cargo build")
    output = outputs.pop()
    if not output.is_relative_to(target):
        raise ValueError("SDL3 output escapes the current Cargo target directory")
    config = output / "lib/cmake/SDL3"
    if not (config / "SDL3Config.cmake").is_file():
        raise ValueError("current SDL3 output has no CMake config")
    version = (config / "SDL3ConfigVersion.cmake").read_text()
    if not re.search(r'set\(PACKAGE_VERSION\s+"3\.4\.18"\)', version):
        raise ValueError("current SDL3 output is not exactly 3.4.18")
    return config


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("report", type=Path)
    parser.add_argument("target", type=Path)
    args = parser.parse_args()
    print(select(args.report.read_text(), args.target))
