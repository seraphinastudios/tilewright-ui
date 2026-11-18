"""The publish workflow's checks, with git local and every service faked."""
import argparse
import hashlib
import http.client
import io
import json
import os
import shutil
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
import unittest.mock
import urllib.error

sys.path.insert(0, str(Path(__file__).resolve().parent))
import publish  # noqa: E402
from release import CRATES, Refused  # noqa: E402

DIGESTS = {name: f"{index:064x}" for index, name in enumerate(CRATES)}
REPOSITORY = "seraphinastudios/tilewright-ui"


class Truncated(io.BytesIO):
    """A body whose connection drops mid-read."""

    def read(self, *args):
        raise http.client.IncompleteRead(b'{"vers"')


class Index(unittest.TestCase):
    def test_paths(self):
        self.assertEqual(publish.index_path("tilewright-ui"), "ti/le/tilewright-ui")
        self.assertEqual(publish.index_path("abc"), "3/a/abc")
        self.assertEqual(publish.index_path("Ab"), "2/ab")

    def opener(self, status=200, lines=(), raw=None, error=None):
        def open_(request, timeout):
            self.assertTrue(request.full_url.startswith("https://index.crates.io/ti/le/"))
            if error is not None:
                raise error
            if status != 200:
                raise urllib.error.HTTPError(request.full_url, status, "", {}, io.BytesIO(b""))
            if isinstance(raw, io.BytesIO):
                return raw
            return io.BytesIO(raw if raw is not None else "\n".join(json.dumps(line) for line in lines).encode())
        return open_

    def test_registry_states(self):
        self.assertIsNone(publish.registry("tilewright-ui", "0.1.0", self.opener(404)))
        self.assertEqual(publish.registry("tilewright-ui", "0.1.0", self.opener(lines=[{"vers": "0.0.1"}])), {})
        entry = {"vers": "0.1.0", "cksum": "a"}
        self.assertEqual(publish.registry("tilewright-ui", "0.1.0", self.opener(lines=[entry])), entry)

    def test_failures_are_never_read_as_missing(self):
        unavailable = [self.opener(503), self.opener(429), self.opener(error=urllib.error.URLError("down")),
                       self.opener(error=TimeoutError()), self.opener(raw=b"<html>captive</html>"),
                       self.opener(raw=b"[1]"), self.opener(raw=b"\xff"),
                       self.opener(error=http.client.IncompleteRead(b"{")),
                       self.opener(error=http.client.BadStatusLine("x")), self.opener(raw=Truncated())]
        for opener in unavailable:
            with self.assertRaises(publish.Unavailable):
                publish.registry("tilewright-ui", "0.1.0", opener)
        with self.assertRaises(Refused) as caught:
            publish.registry("tilewright-ui", "0.1.0", self.opener(403))
        self.assertNotIsInstance(caught.exception, publish.Unavailable)


class Decide(unittest.TestCase):
    def states(self, **overrides):
        states = {name: {"vers": "0.1.0", "cksum": DIGESTS[name]} for name in CRATES}
        states.update({name.replace("_", "-"): state for name, state in overrides.items()})
        return states

    def test_trusted_publishing_refuses_new_names(self):
        with self.assertRaisesRegex(Refused, "first publication pending.*tilewright-app"):
            publish.decide("0.1.0", DIGESTS, self.states(tilewright_app=None), "trusted-publishing")

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


class Environment(unittest.TestCase):
    GOOD = {"can_admins_bypass": False,
            "deployment_branch_policy": {"protected_branches": False, "custom_branch_policies": True}}
    MAIN = {"branch_policies": [{"name": "main", "type": "branch"}]}

    def reader(self, environment=GOOD, policies=MAIN):
        def read(path):
            if environment is None:
                raise subprocess.CalledProcessError(1, "gh", stderr="gh: Not Found (HTTP 404)\n")
            return policies if path.endswith("deployment-branch-policies") else environment
        return read

    def test_main_only_without_bypass_passes(self):
        self.assertIn("admits only main", publish.check_environment(REPOSITORY, self.reader()))

    def test_refusals(self):
        bypass = {**self.GOOD, "can_admins_bypass": True}
        protected = {**self.GOOD, "deployment_branch_policy": {"protected_branches": True, "custom_branch_policies": False}}
        unrestricted = {**self.GOOD, "deployment_branch_policy": None}
        cases = [self.reader(None), self.reader(bypass), self.reader(protected), self.reader(unrestricted),
                 self.reader(policies={"branch_policies": []}),
                 self.reader(policies={"branch_policies": [{"name": "main", "type": "tag"}]}),
                 self.reader(policies={"branch_policies": [{"name": "main", "type": "branch"},
                                                           {"name": "dev", "type": "branch"}]}),
                 self.reader(policies={"branch_policies": [{"name": "*", "type": "branch"}]})]
        for read in cases:
            with self.assertRaisesRegex(Refused, "crates-io-release"):
                publish.check_environment(REPOSITORY, read)

    def failing(self, stderr):
        def read(path):
            raise subprocess.CalledProcessError(1, "gh", stderr=stderr)
        return read

    def test_missing_environment_is_told_apart_from_other_failures(self):
        with self.assertRaisesRegex(Refused, r"does not exist \(gh: Not Found \(HTTP 404\)\): create it as step 2"):
            publish.check_environment(REPOSITORY, self.failing("gh: Not Found (HTTP 404)\n"))
        for stderr in ("gh: Resource not accessible by integration (HTTP 403)\n", "gh: Bad Gateway (HTTP 502)", None):
            with self.assertRaisesRegex(Refused, "could not read the environment .*run again once") as caught:
                publish.check_environment(REPOSITORY, self.failing(stderr))
            if stderr:
                self.assertIn(stderr.split("(")[0].strip(), str(caught.exception))

        def garbled(path):
            raise ValueError("not JSON")
        with self.assertRaisesRegex(Refused, "unreadable .*run again once"):
            publish.check_environment(REPOSITORY, garbled)


class Verify(unittest.TestCase):
    """verify() against a local repository tagged v0.1.0 and faked services."""

    def setUp(self):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.root = Path(directory.name) / "repo"
        self.root.mkdir()
        for args in (("init", "-q", "-b", "main"), ("config", "user.email", "t@example.invalid"),
                     ("config", "user.name", "T"), ("config", "commit.gpgsign", "false"),
                     ("config", "tag.gpgsign", "false")):
            self.git(*args)
        (self.root / "Cargo.toml").write_text('[workspace.package]\nversion = "0.1.0"\n')
        self.git("add", "Cargo.toml")
        self.git("commit", "-q", "-m", "feat: start")
        self.git("tag", "v0.1.0")
        self.base = Path(directory.name)
        self.tree = self.worktree("v0.1.0")
        self.work = Path(directory.name) / "work"
        cwd = os.getcwd()
        os.chdir(self.root)
        self.addCleanup(os.chdir, cwd)
        environment = dict(os.environ, GITHUB_REPOSITORY=REPOSITORY)
        patcher = unittest.mock.patch.dict(os.environ, environment)
        patcher.start()
        self.addCleanup(patcher.stop)
        self.files = {name: f"crate {name}".encode() for name in CRATES}
        self.repackaged = dict(self.files)
        self.sums = {f"{name}-0.1.0.crate": hashlib.sha256(data).hexdigest() for name, data in self.files.items()}
        self.release = {"draft": False, "prerelease": False, "immutable": True,
                        "assets": [{"name": f, "digest": f"sha256:{d}"} for f, d in self.sums.items()]}
        self.environment = Environment.GOOD
        self.policies = Environment.MAIN
        self.calls = []
        self.states = {name: None for name in CRATES}

    def git(self, *args):
        return subprocess.run(("git", "-C", str(self.root)) + args, check=True, capture_output=True,
                              text=True).stdout.strip()

    def worktree(self, ref):
        path = self.base / f"tree-{ref.replace('/', '-')}"
        self.git("worktree", "add", "-q", "--detach", str(path), f"{ref}^{{commit}}")
        return path

    def run_(self, *args, cwd=None, env=None):
        self.calls.append(args)
        if args[:3] == ("gh", "release", "download"):
            target = Path(args[args.index("-D") + 1])
            for name, data in self.files.items():
                (target / f"{name}-0.1.0.crate").write_bytes(data)
            (target / "SHA256SUMS").write_text("".join(f"{d}  {f}\n" for f, d in self.sums.items()))
        elif args[:2] == ("cargo", "package"):
            package = Path(env["CARGO_TARGET_DIR"]) / "package"
            package.mkdir(parents=True)
            for name, data in self.repackaged.items():
                (package / f"{name}-0.1.0.crate").write_bytes(data)
        return ""

    def read(self, path):
        self.calls.append(("read", path))
        if path.endswith("/deployment-branch-policies"):
            return self.policies
        if "/environments/" in path:
            if self.environment is None:
                raise subprocess.CalledProcessError(1, "gh", stderr="gh: Not Found (HTTP 404)")
            return self.environment
        return self.release

    def verify(self, operation="verify", mode="first-publication-token"):
        args = argparse.Namespace(tag="v0.1.0", tree=str(self.tree), work=str(self.work), mode=mode,
                                  operation=operation)
        return publish.verify(args, run=self.run_, index=lambda name, version: self.states[name], read=self.read)

    def test_dry_run_checks_everything_but_the_upload(self):
        plan = self.verify()
        self.assertEqual(plan["pending"], list(CRATES))
        self.assertEqual(len(plan["checked"]), 3)
        attestations = [call for call in self.calls if call[:3] == ("gh", "attestation", "verify")]
        self.assertEqual(len(attestations), 3)
        message = publish.summary(plan)
        self.assertIn("to upload, in order: tilewright-ui tilewright-app tilewright-ui-render", message)
        self.assertIn("admits only main", message)
        self.assertIn("Not tested by a verify run", message)

    def test_provenance_is_pinned_to_release_yml_on_main_at_the_tagged_commit(self):
        self.verify()
        revision = self.git("rev-parse", "v0.1.0^{commit}")
        attestations = [call for call in self.calls if call[:3] == ("gh", "attestation", "verify")]
        self.assertEqual([Path(call[3]).name for call in attestations], [f"{name}-0.1.0.crate" for name in CRATES])
        for call in attestations:
            self.assertEqual(call[4:], (
                "--repo", REPOSITORY,
                "--cert-identity",
                f"https://github.com/{REPOSITORY}/.github/workflows/release.yml@refs/heads/main",
                "--source-ref", "refs/heads/main", "--source-digest", revision, "--deny-self-hosted-runners"))
        package = [call for call in self.calls if call[:2] == ("cargo", "package")]
        self.assertEqual(package, [("cargo", "package", "--locked", "--no-verify",
                                    *[option for name in CRATES for option in ("-p", name)])])

    def test_a_tag_off_main_is_refused(self):
        self.git("checkout", "-q", "-b", "side")
        self.git("commit", "-q", "--allow-empty", "-m", "feat: side")
        self.git("tag", "v0.2.0")
        self.git("checkout", "-q", "main")
        args = argparse.Namespace(tag="v0.2.0", tree=str(self.worktree("v0.2.0")), work=str(self.work),
                                  mode="trusted-publishing", operation="verify")
        with self.assertRaisesRegex(Refused, "v0.2.0 is not on main"):
            publish.verify(args, run=self.run_, index=None, read=self.read)
        self.assertEqual(self.calls, [])

    def test_a_tree_that_is_not_the_tag_is_refused(self):
        self.git("commit", "-q", "--allow-empty", "-m", "feat: later")
        self.tree = self.worktree("main")
        with self.assertRaisesRegex(Refused, "is not a checkout of v0.1.0"):
            self.verify()
        self.assertEqual(self.calls, [])

    def test_digests_are_compared_in_full(self):
        file = "tilewright-ui-render-0.1.0.crate"
        good = self.sums[file]
        last = "0" if good[-1] != "0" else "1"
        self.sums[file] = good[:-1] + last
        with self.assertRaisesRegex(Refused, "SHA256SUMS"):
            self.verify()
        self.sums[file] = good
        self.release["assets"] = [{**asset, "digest": f"sha256:{good[:-1]}{last}"} if asset["name"] == file else asset
                                  for asset in self.release["assets"]]
        self.work = self.work.with_name("work2")
        with self.assertRaisesRegex(Refused, "asset digest"):
            self.verify()
        self.release["assets"] = [{**asset, "digest": f"sha256:{good}"} if asset["name"] == file else asset
                                  for asset in self.release["assets"]]
        self.repackaged["tilewright-ui-render"] = self.files["tilewright-ui-render"] + b" "
        self.work = self.work.with_name("work3")
        with self.assertRaisesRegex(Refused, "differs from the attested"):
            self.verify()

    def test_environment_is_checked_first_for_both_operations(self):
        self.environment = None
        for operation in ("verify", "publish"):
            self.calls.clear()
            with self.assertRaisesRegex(Refused, "does not exist"):
                self.verify(operation)
            self.assertFalse([call for call in self.calls if call[0] != "read"])
        self.environment = {**Environment.GOOD, "can_admins_bypass": True}
        with self.assertRaisesRegex(Refused, "admit only the branch main"):
            self.verify("publish")

    def test_release_must_be_published_immutable_and_stable(self):
        for change in ({"immutable": False}, {"draft": True}, {"prerelease": True}, {"immutable": None}):
            self.release = {**self.release, **change}
            with self.assertRaisesRegex(Refused, "immutable"):
                self.verify()
            self.release.update({"draft": False, "prerelease": False, "immutable": True})

    def test_sums_and_asset_digests_must_match(self):
        file = "tilewright-app-0.1.0.crate"
        good = self.sums[file]
        self.sums[file] = "0" * 64
        with self.assertRaisesRegex(Refused, "SHA256SUMS"):
            self.verify()
        self.sums[file] = good
        self.release["assets"] = [{**asset, "digest": "sha256:" + "0" * 64} if asset["name"] == file else asset
                                  for asset in self.release["assets"]]
        self.work = self.work.with_name("work2")
        with self.assertRaisesRegex(Refused, "asset digest"):
            self.verify()

    def test_repackaged_bytes_must_equal_the_attested_ones(self):
        self.repackaged["tilewright-ui-render"] = b"other"
        with self.assertRaisesRegex(Refused, "differs from the attested"):
            self.verify()
        del self.repackaged["tilewright-ui-render"]
        self.work = self.work.with_name("work2")
        with self.assertRaisesRegex(Refused, "differs from the attested"):
            self.verify()

    def test_a_release_that_does_not_own_its_version_stops_before_any_check(self):
        self.git("commit", "-q", "--allow-empty", "-m", "fix: later")
        self.git("tag", "v0.2.0")
        args = argparse.Namespace(tag="v0.2.0", tree=str(self.worktree("v0.2.0")), work=str(self.work),
                                  mode="trusted-publishing", operation="publish")
        plan = publish.verify(args, run=self.run_, index=None, read=self.read)
        self.assertFalse(plan["owner"])
        self.assertEqual(self.calls, [])


class Confirm(unittest.TestCase):
    PLAN = {"version": "0.1.0", "digests": DIGESTS}

    def confirm(self, answers):
        """Each answer serves one round: an exception, or a function of the crate name."""
        answers = iter(answers)
        sleeps = []

        def run(name, version):
            nonlocal current
            if name == CRATES[0]:
                current = next(answers)
            if isinstance(current, Exception):
                raise current
            return current(name)

        current = None
        result = publish.confirm(self.PLAN, index=run, sleep=sleeps.append, delays=(1, 2, 3))
        return result, sleeps

    @staticmethod
    def listed(name):
        return {"vers": "0.1.0", "cksum": DIGESTS[name]}

    def test_waits_through_unavailable_and_unlisted_answers(self):
        result, sleeps = self.confirm([publish.Unavailable("down"), lambda name: {}, Confirm.listed])
        self.assertIn("attested checksums", result)
        self.assertEqual(sleeps, [1, 2])

    def test_other_bytes_fail_at_once(self):
        with self.assertRaisesRegex(Refused, "yank it"):
            self.confirm([lambda name: {"vers": "0.1.0", "cksum": "f"}])

    def test_gives_up_with_a_clear_message(self):
        with self.assertRaisesRegex(Refused, "gave up waiting.*down"):
            self.confirm([publish.Unavailable("down")] * 4)


class Upload(unittest.TestCase):
    """upload.sh against a fake cargo that records its command lines."""

    def setUp(self):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.temp = Path(directory.name)
        (self.temp / "release").mkdir()
        bin_ = self.temp / "bin"
        bin_.mkdir()
        self.log = self.temp / "cargo.log"
        (bin_ / "cargo").write_text(f'#!/bin/sh\nprintf "%s\\n" "$*" >>"{self.log}"\n')
        (bin_ / "cargo").chmod(0o755)
        self.env = {"PATH": f"{bin_}:{os.environ['PATH']}", "RUNNER_TEMP": str(self.temp),
                    "CARGO_REGISTRY_TOKEN": "secret-token-value"}
        self.script = Path(__file__).resolve().parent / "upload.sh"

    def upload(self, pending, **env):
        return subprocess.run(["bash", str(self.script)], env={**self.env, "PENDING": pending, **env},
                              capture_output=True, text=True)

    def calls(self):
        return self.log.read_text().splitlines() if self.log.exists() else []

    def test_publishes_in_order_without_the_token_on_a_command_line(self):
        result = self.upload("tilewright-ui tilewright-app tilewright-ui-render")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.calls(), [f"publish --locked --no-verify -p {name}" for name in CRATES])
        self.assertNotIn("secret-token-value", self.log.read_text())
        self.assertNotIn("--token", self.script.read_text())

    def test_refuses_a_name_outside_the_three_before_publishing_anything(self):
        for pending in ("tilewright-ui evil", "tilewright-ui-renderx", "tilewright-*", "../x"):
            result = self.upload(pending)
            self.assertNotEqual(result.returncode, 0, pending)
            self.assertEqual(self.calls(), [], pending)

    def test_refuses_without_a_token(self):
        result = self.upload("tilewright-ui", CARGO_REGISTRY_TOKEN="")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("no crates.io token", result.stdout + result.stderr)
        self.assertEqual(self.calls(), [])


if __name__ == "__main__":
    unittest.main()
