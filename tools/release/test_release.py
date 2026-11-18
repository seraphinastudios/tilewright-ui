"""Release decisions against small throwaway repositories."""
import io
import json
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent))
import release  # noqa: E402


class Repository(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.root = Path(directory.name)
        self.git("init", "-q", "-b", "main")
        self.git("config", "user.email", "test@example.invalid")
        self.git("config", "user.name", "Test")
        self.git("config", "commit.gpgsign", "false")
        self.git("config", "tag.gpgsign", "false")

    def git(self, *args):
        return subprocess.run(("git", "-C", str(self.root)) + args, check=True,
                              capture_output=True, text=True).stdout.strip()

    def commit(self, message, version="0.1.0"):
        (self.root / "Cargo.toml").write_text(f'[workspace.package]\nversion = "{version}"\n')
        self.git("add", "Cargo.toml")
        self.git("commit", "-q", "--allow-empty", "-m", message)
        return self.git("rev-parse", "HEAD")

    def plan(self, event="push"):
        return release.plan("HEAD", event, self.root)


class Plan(Repository):
    def test_merge_never_cuts_the_first_release(self):
        self.commit("feat: start")
        result = self.plan()
        self.assertFalse(result["release"])
        self.assertIn("run the Release workflow on main", result["reason"])

    def test_dispatch_cuts_the_first_release_which_owns_the_crate_version(self):
        self.commit("feat: start")
        result = self.plan("workflow_dispatch")
        self.assertEqual((result["version"], result["tag"]), ("0.1.0", "v0.1.0"))
        self.assertEqual((result["crate_version"], result["crate_owner"], result["crate_owner_tag"]),
                         ("0.1.0", True, "v0.1.0"))

    def test_every_typed_commit_bumps_the_minor_version(self):
        self.commit("feat: start")
        self.git("tag", "v0.3.0")
        self.commit("docs: wording (#4)\n\nBody.")
        result = self.plan()
        self.assertEqual(result["version"], "0.4.0")
        self.assertEqual(result["previous_tag"], "v0.3.0")
        self.assertFalse(result["crate_owner"])
        self.assertEqual(result["crate_owner_tag"], "v0.3.0")

    def test_untyped_commits_do_not_release(self):
        self.commit("feat: start")
        self.git("tag", "v0.1.0")
        self.commit("Update README")
        self.assertFalse(self.plan()["release"])
        self.commit("Revert \"feat: start\"")
        self.assertTrue(self.plan()["release"])

    def test_breaking_note_releases_an_untyped_commit(self):
        self.commit("feat: start")
        self.git("tag", "v0.1.0")
        self.commit("Rework\n\n* BREAKING CHANGE: the field moved")
        self.assertTrue(self.plan()["release"])

    def test_typed_title_rules(self):
        for title in ("feat(ui)!: x", "build(deps): bump a", "ci: y"):
            self.assertTrue(release.releases(title), title)
        for title in ("Feat: x", "feat:x", "feat", "fix(: x", ""):
            self.assertFalse(release.releases(title), title)

    def test_crate_version_bump_is_owned_by_its_release(self):
        self.commit("feat: start")
        self.git("tag", "v0.1.0")
        self.commit("feat: more")
        self.git("tag", "v0.2.0")
        self.commit("feat!: break", version="0.2.0")
        result = self.plan()
        self.assertEqual((result["version"], result["crate_version"], result["crate_owner"]),
                         ("0.3.0", "0.2.0", True))

    def test_owner_is_the_earliest_release_at_the_crate_version(self):
        self.commit("feat: start")
        self.git("tag", "v0.1.0")
        self.commit("feat: bump", version="0.1.1")
        self.git("tag", "v0.2.0")
        self.commit("fix: more", version="0.1.1")
        self.git("tag", "v0.3.0")
        self.commit("fix: again", version="0.1.1")
        self.assertEqual(self.plan()["crate_owner_tag"], "v0.2.0")
        revision = self.git("rev-parse", "v0.3.0")
        previous = release.previous_tag(revision, self.root, below=(0, 3, 0))
        self.assertEqual(release.ownership(revision, "v0.3.0", previous, self.root)["crate_owner_tag"], "v0.2.0")
        revision = self.git("rev-parse", "v0.2.0")
        previous = release.previous_tag(revision, self.root, below=(0, 2, 0))
        self.assertTrue(release.ownership(revision, "v0.2.0", previous, self.root)["crate_owner"])

    def test_changed_crate_content_without_a_bump_is_reported(self):
        self.commit("feat: start")
        self.git("tag", "v0.1.0")
        self.commit("docs: nothing packaged")
        self.assertFalse(self.plan()["crate_changed"])
        source = self.root / "crates/tilewright-app/tests"
        source.mkdir(parents=True)
        (source / "t.rs").write_text("")
        self.git("add", ".")
        self.commit("test: only tests")
        self.assertFalse(self.plan()["crate_changed"])
        (self.root / "crates/tilewright-app/lib.rs").write_text("")
        self.git("add", ".")
        self.commit("feat: packaged change")
        self.assertTrue(self.plan()["crate_changed"])
        self.commit("feat: bump", version="0.1.1")
        self.assertFalse(self.plan()["crate_changed"])

    def test_lower_crate_version_stops_the_plan(self):
        self.commit("feat: start", version="0.2.0")
        self.git("tag", "v0.1.0")
        self.commit("fix: back", version="0.1.9")
        with self.assertRaises(release.Refused):
            self.plan()

    def test_tags_that_are_not_release_versions_are_ignored(self):
        self.commit("feat: start")
        self.git("tag", "v0.1")
        self.git("tag", "v01.0.0")
        self.assertIsNone(release.previous_tag("HEAD", self.root))


class CheckVersion(Repository):
    def test_steps(self):
        base = self.commit("feat: start", version="0.1.0")
        for version in ("0.1.0", "0.1.1", "0.2.0"):
            self.commit("chore: v", version=version)
            self.assertIn(version, release.check_version(base, self.root))
        for version in ("0.1.2", "0.3.0", "0.0.9", "1.0.0", "0.2.1"):
            self.commit("chore: v", version=version)
            with self.assertRaises(release.Refused, msg=version):
                release.check_version(base, self.root)


class CheckCrates(Repository):
    def crate(self, directory, name, version, sha, extra=(), dirty=False, package=None):
        files = {"Cargo.toml": f'[package]\nname = "{package or name}"\nversion = "{version}"\n',
                 "README.md": "r", "LICENSE": "l", "src/lib.rs": "",
                 ".cargo_vcs_info.json": json.dumps({"git": {"sha1": sha, "dirty": dirty}})}
        with tarfile.open(directory / f"{name}-{version}.crate", "w:gz") as archive:
            for path, content in [*files.items(), *extra]:
                data = content.encode()
                info = tarfile.TarInfo(path if path.startswith("/") or ".." in path else f"{name}-{version}/{path}")
                info.size = len(data)
                archive.addfile(info, io.BytesIO(data))

    def setUp(self):
        super().setUp()
        self.sha = self.commit("feat: start")
        self.out = self.root / "package"
        self.out.mkdir()

    def test_three_clean_crates_pass(self):
        for name in release.CRATES:
            self.crate(self.out, name, "0.1.0", self.sha)
        self.assertIn("0.1.0", release.check_crates(self.out, "HEAD", self.root))

    def test_refusals(self):
        cases = {
            "missing": lambda name: name != "tilewright-app" and self.crate(self.out, name, "0.1.0", self.sha),
            "dirty": lambda name: self.crate(self.out, name, "0.1.0", self.sha, dirty=True),
            "other revision": lambda name: self.crate(self.out, name, "0.1.0", "0" * 40),
            "other version": lambda name: self.crate(self.out, name, "0.1.1", self.sha),
            "escape": lambda name: self.crate(self.out, name, "0.1.0", self.sha, extra=[("../x", "")]),
            "other package": lambda name: self.crate(self.out, name, "0.1.0", self.sha, package="other"),
        }
        for case, make in cases.items():
            for path in self.out.iterdir():
                path.unlink()
            for name in release.CRATES:
                make(name)
            with self.assertRaises(release.Refused, msg=case):
                release.check_crates(self.out, "HEAD", self.root)


if __name__ == "__main__":
    unittest.main()
