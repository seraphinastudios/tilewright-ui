"""Adversarial manifest cases for the dependency boundary policy."""
import importlib.util
from pathlib import Path
import tempfile
import unittest

SPEC = importlib.util.spec_from_file_location(
    "check_architecture", Path(__file__).resolve().parents[2] / "tools/check_architecture.py")
POLICY = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(POLICY)


class Boundaries(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        (self.root / "Cargo.toml").write_text('[workspace]\nmembers = []\n')
        for name in ("tilewright-ui", "tilewright-app", "tilewright-ui-render", "tilewright-ui-ffi"):
            path = self.root / "crates" / name
            (path / "src").mkdir(parents=True)
            (path / "src/lib.rs").write_text('#![forbid(unsafe_code)]\n')
            (path / "Cargo.toml").write_text(
                f'[package]\nname = "{name}"\npublish = false\n')

    def add(self, text, name="tilewright-ui"):
        with (self.root / "crates" / name / "Cargo.toml").open("a") as out:
            out.write(text)

    def test_minimal_graph(self):
        self.assertEqual(POLICY.check(self.root), [])

    def test_target_dev_dependency_cannot_hide_network(self):
        self.add('\n[target.\'cfg(unix)\'.dev-dependencies]\nnet = { package = "reqwest", version = "=1.0.0" }\n')
        self.assertTrue(any("forbidden edge tilewright-ui -> reqwest" in e for e in POLICY.check(self.root)))

    def test_build_dependency_cannot_reverse_application_edge(self):
        self.add('\n[build-dependencies]\ntilewright-app = { path = "../tilewright-app" }\n')
        self.assertTrue(any("forbidden edge" in e for e in POLICY.check(self.root)))

    def test_git_and_external_path_are_rejected(self):
        self.add('\n[dependencies]\nunicode-segmentation = { version = "=1.13.3", git = "https://example.invalid/source", path = "../../../outside" }\n')
        errors = POLICY.check(self.root)
        self.assertTrue(any("external path" in e for e in errors))
        self.assertTrue(any("reviewed crates.io" in e for e in errors))

    def test_local_dependency_identity_is_checked(self):
        self.add('\n[dependencies]\ntilewright-ui = { path = "../tilewright-app" }\n', "tilewright-app")
        self.assertTrue(any("local workspace package" in e for e in POLICY.check(self.root)))

    def test_unpinned_registry_version_is_rejected(self):
        self.add('\n[dependencies]\nunicode-segmentation = "1.13"\n')
        self.assertTrue(any("one exact version" in e for e in POLICY.check(self.root)))

    def test_inherited_dependencies_are_checked(self):
        (self.root / "Cargo.toml").write_text('[workspace.dependencies]\nnet = { package = "reqwest", version = "=1.0.0" }\n')
        self.add('\n[dependencies]\nnet.workspace = true\n')
        self.assertTrue(any("forbidden edge" in e for e in POLICY.check(self.root)))

    def test_renderer_override_is_rejected(self):
        with (self.root / "Cargo.toml").open("a") as out:
            out.write('\n[patch.crates-io]\ntilewright-scene = { path = "../renderer" }\n')
        self.assertTrue(any("overrides" in e for e in POLICY.check(self.root)))

    def test_unsafe_policy_is_required(self):
        (self.root / "crates/tilewright-ui/src/lib.rs").write_text('')
        self.assertTrue(any("forbid unsafe" in e for e in POLICY.check(self.root)))

    def test_unused_workspace_override_is_rejected(self):
        (self.root / "Cargo.toml").write_text('[workspace.dependencies]\nunused = { path = "../outside" }\n')
        self.assertTrue(any("escapes or misses" in e for e in POLICY.check(self.root)))

    def test_renderer_does_not_depend_on_app(self):
        self.add('\n[dependencies]\ntilewright-app = { path = "../tilewright-app" }\n', "tilewright-ui-render")
        self.assertTrue(any("forbidden edge" in e for e in POLICY.check(self.root)))

    def test_renderer_recovery_cohort_is_pinned(self):
        self.add('\n[dependencies]\ntilewright-scene = "=0.6.1"\n', "tilewright-ui-render")
        self.assertEqual(POLICY.check(self.root), [])
        path = self.root / "crates/tilewright-ui-render/Cargo.toml"
        path.write_text(path.read_text().replace("=0.6.1", "=0.6.0"))
        self.assertTrue(any("must retain =0.6.1" in e for e in POLICY.check(self.root)))

    def test_unused_workspace_renderer_pin_is_checked(self):
        (self.root / "Cargo.toml").write_text('[workspace.dependencies]\ntilewright-render-sdl3 = "=0.6.0"\n')
        self.assertTrue(any("must use =0.6.1" in e for e in POLICY.check(self.root)))


if __name__ == "__main__":
    unittest.main()
