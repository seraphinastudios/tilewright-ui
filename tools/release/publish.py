#!/usr/bin/env python3
"""Check a release's crate archives before and after a crates.io upload.

verify   authenticate the release's three .crate assets (immutable release,
         SHA256SUMS, GitHub's asset digests, build provenance from
         release.yml on main at the tagged commit), package the tagged
         commit again and require the same bytes, read crates.io's index,
         and decide which crates still need uploading. Fails closed, with
         nothing uploaded, when the credential cannot publish what is pending.
confirm  wait until crates.io's index lists every crate at the version with
         the attested archive's checksum.

Standard library, the runner's GitHub CLI and the pinned Cargo only. It never
reads a registry token. docs/RELEASING.md describes the workflow around it.
"""
import argparse
import hashlib
import http.client
import json
import os
from pathlib import Path
import subprocess
import sys
import time
import urllib.error
import urllib.request

sys.path.insert(0, str(Path(__file__).resolve().parent))
from release import CRATES, Refused, TAG, git, ownership, parse, previous_tag  # noqa: E402

ENVIRONMENT = "crates-io-release"
CHECKLIST = "docs/CRATE-FIRST-PUBLICATION.md"
INDEX = "https://index.crates.io/"


def index_path(name):
    """A crate's file in crates.io's sparse index."""
    name = name.lower()
    if len(name) <= 2:
        return f"{len(name)}/{name}"
    if len(name) == 3:
        return f"3/{name[0]}/{name}"
    return f"{name[:2]}/{name[2:4]}/{name}"


class Unavailable(Refused):
    """crates.io's index did not give a usable answer; worth asking again."""


def registry(name, version, opener=urllib.request.urlopen):
    """None when crates.io has no such crate, else the version's index entry or {}.

    Only a 404 means "no such crate": any other failure raises, never None."""
    request = urllib.request.Request(INDEX + index_path(name), headers={
        "User-Agent": "tilewright-ui-publish (https://github.com/seraphinastudios/tilewright-ui)",
        "Cache-Control": "no-cache"})
    try:
        with opener(request, timeout=30) as response:
            lines = response.read().decode().splitlines()
        entries = [json.loads(line) for line in lines if line.strip()]
        if not all(isinstance(entry, dict) for entry in entries):
            raise ValueError("not an index file")
    except urllib.error.HTTPError as error:
        error.close()
        if error.code == 404:
            return None
        if error.code >= 500 or error.code == 429:
            raise Unavailable(f"crates.io's index answered {error.code} for {name}") from None
        raise Refused(f"crates.io's index answered {error.code} for {name}") from None
    except (urllib.error.URLError, TimeoutError, OSError, http.client.HTTPException) as error:
        raise Unavailable(f"crates.io's index did not answer for {name}: {error}") from None
    except (UnicodeDecodeError, ValueError):
        raise Unavailable(f"crates.io's index sent something other than an index file for {name}") from None
    return next((entry for entry in entries if entry.get("vers") == version), {})


def decide(version, digests, states, mode):
    """Which crates to upload, in order; refuses what the credential cannot do."""
    pending, absent = [], []
    for name in CRATES:
        state = states[name]
        if state is None:
            absent.append(name)
        elif state:
            if state.get("cksum") != digests[name]:
                raise Refused(f"crates.io has {name} {version} with other bytes than this release's")
            if state.get("yanked"):
                raise Refused(f"crates.io has {name} {version}, yanked")
            continue
        pending.append(name)
    if mode == "trusted-publishing" and absent:
        raise Refused(f"first publication pending: crates.io has no crate named {', '.join(absent)} yet, and "
                      f"trusted publishing cannot create a crate. Nothing was uploaded. Follow {CHECKLIST}.")
    if mode == "first-publication-token" and any(name not in absent for name in pending):
        raise Refused("the first-publication token publishes only crate names crates.io lacks; "
                      f"{', '.join(name for name in pending if name not in absent)} already exist. "
                      "Use credential trusted-publishing. Nothing was uploaded.")
    return pending


def sha256(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def run(*args, **kwargs):
    return subprocess.run(args, check=True, capture_output=True, text=True, **kwargs).stdout


def gh_json(*args, run=run):
    return json.loads(run("gh", "api", *args))


def check_environment(repository, read=gh_json):
    """The environment exists, admits only main and has no administrator bypass.

    A job that names a missing environment would create it with no branch rule,
    so a publish run checks this before its publish job names the environment."""
    try:
        environment = read(f"repos/{repository}/environments/{ENVIRONMENT}")
        policies = read(f"repos/{repository}/environments/{ENVIRONMENT}/deployment-branch-policies")
    except subprocess.CalledProcessError as error:
        detail = " ".join(str(error.stderr or "").split()) or f"exit status {error.returncode}"
        if "HTTP 404" in detail:
            raise Refused(f"the environment {ENVIRONMENT} does not exist ({detail}): create it as step 2 of "
                          f"{CHECKLIST} says") from None
        raise Refused(f"could not read the environment {ENVIRONMENT} ({detail}): run again once; if it "
                      f"repeats, ask (see {CHECKLIST})") from None
    except ValueError:
        raise Refused(f"GitHub sent something unreadable for the environment {ENVIRONMENT}: run again once; "
                      f"if it repeats, ask (see {CHECKLIST})") from None
    rule = environment.get("deployment_branch_policy") or {}
    names = [(policy.get("name"), policy.get("type")) for policy in policies.get("branch_policies", [])]
    if environment.get("can_admins_bypass") is not False or rule != {
            "protected_branches": False, "custom_branch_policies": True} or names != [("main", "branch")]:
        raise Refused(f"the environment {ENVIRONMENT} must admit only the branch main, with no administrator "
                      f"bypass; see {CHECKLIST}")
    return f"the environment {ENVIRONMENT} admits only main, with no administrator bypass"


def check_release(tag, release):
    if release.get("draft") is not False or release.get("prerelease") is not False or release.get("immutable") is not True:
        raise Refused(f"{tag} is not a published, immutable, stable release")


def attested_digests(assets, names, release):
    """Each archive's SHA-256, once SHA256SUMS and GitHub's asset digest agree on it."""
    sums = {}
    for line in (assets / "SHA256SUMS").read_text().splitlines():
        digest, file = line.split(maxsplit=1)
        sums[file] = digest
    recorded = {asset["name"]: asset.get("digest") for asset in release.get("assets", [])}
    digests = {}
    for name, file in names.items():
        digest = sha256(assets / file)
        if sums.get(file) != digest or recorded.get(file) != f"sha256:{digest}":
            raise Refused(f"{file} does not match SHA256SUMS and the release's asset digest")
        digests[name] = digest
    return digests


def check_repackaged(package, names, digests, tag):
    for name, file in names.items():
        path = Path(package) / file
        if not path.is_file() or sha256(path) != digests[name]:
            raise Refused(f"{file} packaged from {tag} differs from the attested release asset")


def verify(args, run=run, index=registry, read=None):
    """Everything short of the upload; returns the plan."""
    read = read or (lambda *path: gh_json(*path, run=run))
    repository = os.environ["GITHUB_REPOSITORY"]
    if not TAG.fullmatch(args.tag):
        raise Refused(f"not a release tag: {args.tag!r}")
    revision = git("rev-parse", f"{args.tag}^{{commit}}").strip()
    if git("rev-parse", "HEAD", root=args.tree).strip() != revision:
        raise Refused(f"{args.tree} is not a checkout of {args.tag}")
    if subprocess.run(("git", "merge-base", "--is-ancestor", revision, "HEAD")).returncode != 0:
        raise Refused(f"{args.tag} is not on main")
    owner = ownership(revision, args.tag, previous_tag(revision, below=parse(args.tag, TAG)))
    version = owner["crate_version"]
    plan = {"tag": args.tag, "revision": revision, "version": version, "owner": owner["crate_owner"],
            "owner_tag": owner["crate_owner_tag"], "pending": [], "digests": {}, "checked": [],
            "operation": args.operation}
    if not owner["crate_owner"]:
        return plan
    # First, for verify too: a dry run proves the environment before any
    # token is stored in it, and a publish run never reaches an unchecked one.
    plan["checked"].append(check_environment(repository, read))
    release = read(f"repos/{repository}/releases/tags/{args.tag}")
    check_release(args.tag, release)
    assets = Path(args.work) / "assets"
    assets.mkdir(parents=True)
    names = {name: f"{name}-{version}.crate" for name in CRATES}
    run("gh", "release", "download", args.tag, "-R", repository, "-D", str(assets), "-p", "SHA256SUMS",
        *[option for file in names.values() for option in ("-p", file)])
    plan["digests"] = attested_digests(assets, names, release)
    identity = f"https://github.com/{repository}/.github/workflows/release.yml@refs/heads/main"
    for file in names.values():
        run("gh", "attestation", "verify", str(assets / file), "--repo", repository, "--cert-identity", identity,
            "--source-ref", "refs/heads/main", "--source-digest", revision, "--deny-self-hosted-runners")
    plan["checked"].append("SHA256SUMS, GitHub's asset digests and build provenance from release.yml on main")
    # `cargo publish` packages again from the same tree with the same Cargo,
    # so a separate packaging run must give exactly the attested bytes.
    target = Path(args.work) / "target"
    run("cargo", "package", "--locked", "--no-verify", *[option for name in CRATES for option in ("-p", name)],
        cwd=args.tree, env={**os.environ, "CARGO_TARGET_DIR": str(target)})
    check_repackaged(target / "package", names, plan["digests"], args.tag)
    plan["checked"].append(f"packaging {args.tag} again on this runner gives the attested bytes")
    states = {name: index(name, version) for name in CRATES}
    plan["pending"] = decide(version, plan["digests"], states, args.mode)
    return plan


def confirm(plan, index=registry, sleep=time.sleep, delays=(5, 10, 15, 20, 30, 30, 30, 30, 30, 30, 30, 30)):
    """Waits, with backoff, until the index lists the attested checksums.

    Other bytes fail at once; an index that does not answer is asked again
    until the waits run out (about five minutes)."""
    problem = "it did not list them yet"
    for delay in (*delays, None):
        try:
            states = {name: index(name, plan["version"]) for name in CRATES}
        except Unavailable as error:
            problem = str(error)
        else:
            for name, state in states.items():
                if state and state.get("cksum") != plan["digests"][name]:
                    raise Refused(f"crates.io has {name} {plan['version']} with other bytes than the attested "
                                  "archive. It is public: yank it, publish the next crate version, and investigate")
            if all(state and state.get("cksum") == plan["digests"][name] for name, state in states.items()):
                return f"crates.io lists {', '.join(CRATES)} at {plan['version']} with the attested checksums"
            problem = "it did not list all three at this version yet"
        if delay is not None:
            sleep(delay)
    raise Refused(f"gave up waiting for crates.io's index ({problem}). Uploads that succeeded stay published; "
                  "dispatch the same run again later to check and finish")


def summary(plan):
    lines = [f"Checked: {item}." for item in plan.get("checked", [])]
    if plan.get("operation") == "verify" and plan["pending"]:
        lines.append("Not tested by a verify run: the crates.io token, its exchange, GitHub admitting the "
                     "publish job to the environment, the publish job's own repackaging, the upload and the "
                     "index confirmation.")
    return "\n".join([headline(plan), *lines])


def headline(plan):
    if not plan["owner"]:
        return (f"{plan['tag']} carries crate version {plan['version']}, which {plan['owner_tag']} owns; "
                "this release publishes no crates.")
    if not plan["pending"]:
        return f"crates.io already has all three crates at {plan['version']} with this release's bytes. Nothing to upload."
    return f"{plan['tag']} owns crate version {plan['version']}; to upload, in order: {' '.join(plan['pending'])}"


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    commands = parser.add_subparsers(dest="command", required=True)
    command = commands.add_parser("verify")
    command.add_argument("--tag", required=True)
    command.add_argument("--tree", required=True, help="a clean checkout of the tag")
    command.add_argument("--work", required=True, help="an absent directory for downloads and packages")
    command.add_argument("--mode", required=True, choices=("trusted-publishing", "first-publication-token"))
    command.add_argument("--operation", required=True, choices=("verify", "publish"))
    command.add_argument("--plan", required=True, help="where to write the plan")
    command = commands.add_parser("confirm")
    command.add_argument("--plan", required=True)
    args = parser.parse_args(argv)
    try:
        if args.command == "verify":
            plan = verify(args)
            Path(args.plan).write_text(json.dumps(plan, sort_keys=True))
            message = summary(plan)
            if "GITHUB_OUTPUT" in os.environ:
                with open(os.environ["GITHUB_OUTPUT"], "a") as output:
                    output.write(f"pending={' '.join(plan['pending'])}\nowner={str(plan['owner']).lower()}\n")
        else:
            message = confirm(json.loads(Path(args.plan).read_text()))
    except (Refused, subprocess.CalledProcessError) as error:
        detail = getattr(error, "stderr", None) or error
        print(f"::error::{str(detail).strip()}", file=sys.stderr)
        return 1
    for line in message.splitlines():
        print(f"::notice::{line}")
    if "GITHUB_STEP_SUMMARY" in os.environ:
        with open(os.environ["GITHUB_STEP_SUMMARY"], "a") as out:
            out.write("\n\n".join(message.splitlines()) + "\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
