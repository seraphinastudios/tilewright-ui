"""Reject stale, ambiguous and foreign SDL CMake prefixes."""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

SPEC = importlib.util.spec_from_file_location(
    "find_sdl_config", Path(__file__).resolve().parents[2] / "tools/find_sdl_config.py")
SELECTOR = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(SELECTOR)


class SdlConfigTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.target = self.root / "target"
        self.target.mkdir()

    def report(self, name="current", version="3.4.18", outside=False):
        out = (self.root if outside else self.target) / name / "out"
        config = out / "lib/cmake/SDL3"
        config.mkdir(parents=True)
        (config / "SDL3Config.cmake").write_text("# fixture")
        (config / "SDL3ConfigVersion.cmake").write_text(f'set(PACKAGE_VERSION "{version}")\n')
        return json.dumps({"reason": "build-script-executed", "package_id": "registry+https://github.com/rust-lang/crates.io-index#sdl3-sys@0.6.6", "out_dir": str(out)}), config

    def test_only_current_report_selects_prefix(self):
        self.report("stale")
        report, config = self.report()
        self.assertEqual(SELECTOR.select(report, self.target), config)

    def test_missing_or_ambiguous_current_outputs_fail(self):
        first, _ = self.report("first")
        second, _ = self.report("second")
        for report in ("", first + "\n" + second):
            with self.subTest(report=report), self.assertRaisesRegex(ValueError, "exactly one"):
                SELECTOR.select(report, self.target)

    def test_foreign_output_and_wrong_sdl_version_fail(self):
        report, _ = self.report(outside=True)
        with self.assertRaisesRegex(ValueError, "escapes"):
            SELECTOR.select(report, self.target)
        report, _ = self.report("old", version="3.4.2")
        with self.assertRaisesRegex(ValueError, "exactly 3.4.18"):
            SELECTOR.select(report, self.target)


if __name__ == "__main__":
    unittest.main()
