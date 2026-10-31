#!/usr/bin/env python3
"""Stage the explicit verified font pack; never download or discover fonts."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil

MANIFEST_SHA = '94b81a712fb41a66eb744c88ae2ed312cc5d0ec1a491e9821fcc278d99410f13'
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--output', type=Path, required=True)
args = parser.parse_args()
source = os.environ.get('TILEWRIGHT_UI_FONT_ROOT')
if not source:
    parser.error('Set TILEWRIGHT_UI_FONT_ROOT to the verified 17-font pack')
root = Path(source).resolve(strict=True)
manifest = (root / 'manifest.json').read_bytes()
if hashlib.sha256(manifest).hexdigest() != MANIFEST_SHA:
    parser.error('Font pack manifest differs from the reviewed immutable artifact')
files = [('manifest.json', len(manifest), MANIFEST_SHA)]
for font in json.loads(manifest)['fonts']:
    files.append((font['path'], font['bytes'], font['sha256']))
    notice = font['notice']
    files.append((notice['path'], notice['bytes'], notice['sha256']))
# Verify the complete input before writing any output.
for name, size, digest in files:
    value = (root / name).read_bytes()
    if len(value) != size or hashlib.sha256(value).hexdigest() != digest:
        parser.error(f'Font pack integrity mismatch: {name}')
for name, _, _ in files:
    destination = args.output / name
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(root / name, destination)
print(f'Staged {len(files)} verified font and notice files to {args.output}')
