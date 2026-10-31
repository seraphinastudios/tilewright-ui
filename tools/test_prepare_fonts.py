"""Additional preparation regressions; no test makes network requests."""
import io
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

from test_font_pack import fixture, pack


class PreparationTests(unittest.TestCase):
    def test_short_download_is_rejected(self):
        class Response(io.BytesIO):
            status = 200

            def geturl(self):
                return self.url

        class ShortRead:
            def open(self, request, timeout):
                response = Response(b"x")
                response.url = request.full_url
                return response

        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            _, manifest, expected = fixture(root)
            with self.assertRaisesRegex(ValueError, "checksum/size"):
                pack.acquire(root / "download", manifest, expected, ShortRead())

    def test_command_requires_explicit_source_and_rejects_source_payload(self):
        script = Path(__file__).with_name("prepare-fonts.py")
        for args in (("/tmp/unused-font-destination",), (str(script.parent / "fonts"), "--acquire")):
            result = subprocess.run([sys.executable, str(script), *args], capture_output=True, text=True)
            self.assertEqual(result.returncode, 2)


if __name__ == "__main__":
    unittest.main()
