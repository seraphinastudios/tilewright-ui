"""The crates.io decisions of the publish workflow, without the network."""
import io
import json
from pathlib import Path
import sys
import unittest
import urllib.error

sys.path.insert(0, str(Path(__file__).resolve().parent))
import publish  # noqa: E402
from release import CRATES, Refused  # noqa: E402

DIGESTS = {name: f"{index:064x}" for index, name in enumerate(CRATES)}


class Index(unittest.TestCase):
    def test_paths(self):
        self.assertEqual(publish.index_path("tilewright-ui"), "ti/le/tilewright-ui")
        self.assertEqual(publish.index_path("abc"), "3/a/abc")
        self.assertEqual(publish.index_path("Ab"), "2/ab")

    def opener(self, status=200, lines=()):
        def open_(request, timeout):
            self.assertTrue(request.full_url.startswith("https://index.crates.io/ti/le/"))
            if status != 200:
                raise urllib.error.HTTPError(request.full_url, status, "", {}, None)
            return io.BytesIO("\n".join(json.dumps(line) for line in lines).encode())
        return open_

    def test_registry_states(self):
        self.assertIsNone(publish.registry("tilewright-ui", "0.1.0", self.opener(404)))
        self.assertEqual(publish.registry("tilewright-ui", "0.1.0", self.opener(lines=[{"vers": "0.0.1"}])), {})
        entry = {"vers": "0.1.0", "cksum": "a"}
        self.assertEqual(publish.registry("tilewright-ui", "0.1.0", self.opener(lines=[entry])), entry)
        with self.assertRaises(Refused):
            publish.registry("tilewright-ui", "0.1.0", self.opener(503))


class Decide(unittest.TestCase):
    def states(self, **overrides):
        states = {name: {"vers": "0.1.0", "cksum": DIGESTS[name]} for name in CRATES}
        states.update({name.replace("_", "-"): state for name, state in overrides.items()})
        return states

    def test_trusted_publishing_refuses_new_names(self):
        for mode in ("trusted-publishing",):
            with self.assertRaisesRegex(Refused, "first publication pending.*tilewright-app"):
                publish.decide("0.1.0", DIGESTS, self.states(tilewright_app=None), mode)

    def test_first_publication_uploads_only_new_names_in_order(self):
        states = {name: None for name in CRATES}
        self.assertEqual(publish.decide("0.1.0", DIGESTS, states, "first-publication-token"), list(CRATES))
        states["tilewright-ui"] = {"vers": "0.1.0", "cksum": DIGESTS["tilewright-ui"]}
        self.assertEqual(publish.decide("0.1.0", DIGESTS, states, "first-publication-token"),
                         ["tilewright-app", "tilewright-ui-render"])
        states["tilewright-ui"] = {}
        with self.assertRaisesRegex(Refused, "only crate names crates.io lacks"):
            publish.decide("0.1.0", DIGESTS, states, "first-publication-token")

    def test_trusted_publishing_uploads_missing_versions(self):
        states = self.states(tilewright_ui_render={})
        self.assertEqual(publish.decide("0.1.0", DIGESTS, states, "trusted-publishing"), ["tilewright-ui-render"])
        self.assertEqual(publish.decide("0.1.0", DIGESTS, self.states(), "trusted-publishing"), [])

    def test_other_bytes_or_yanked_stop(self):
        with self.assertRaisesRegex(Refused, "other bytes"):
            publish.decide("0.1.0", DIGESTS, self.states(tilewright_ui={"vers": "0.1.0", "cksum": "f"}), "trusted-publishing")
        yanked = {"vers": "0.1.0", "cksum": DIGESTS["tilewright-app"], "yanked": True}
        with self.assertRaisesRegex(Refused, "yanked"):
            publish.decide("0.1.0", DIGESTS, self.states(tilewright_app=yanked), "trusted-publishing")

    def test_names_are_load_bearing(self):
        # crates.io's trusted publishers name this workflow file and environment.
        root = Path(__file__).resolve().parents[2]
        workflow = (root / ".github/workflows/publish-crate.yml").read_text()
        self.assertIn(f"environment: {publish.ENVIRONMENT}\n", workflow)
        self.assertEqual(publish.ENVIRONMENT, "crates-io-release")


if __name__ == "__main__":
    unittest.main()
