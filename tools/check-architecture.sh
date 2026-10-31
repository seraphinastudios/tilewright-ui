#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
export PYTHONDONTWRITEBYTECODE=1
python3 tools/check_architecture.py
python3 -m unittest discover -s tests/architecture -p 'test_*.py'
