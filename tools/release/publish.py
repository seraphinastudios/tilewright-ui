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
import json
import os
from pathlib import Path
import re
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


def registry(name, version, opener=urllib.request.urlopen):
    """None when crates.io has no such crate, else the version's index entry or {}."""
    request = urllib.request.Request(INDEX + index_path(name), headers={
        "User-Agent": "tilewright-ui-publish (https://github.com/seraphinastudios/tilewright-ui)",
        "Cache-Control": "no-cache"})
    try:
        with opener(request, timeout=30) as response:
            lines = response.read().decode().splitlines()
    except urllib.error.HTTPError as error:
        if error.code == 404:
            return None
        raise Refused(f"crates.io's index answered {error.code} for {name}") from None
    except (urllib.error.URLError, TimeoutError) as error:
        raise Refused(f"crates.io's index did not answer for {name}: {error}") from None
    entries = [json.loads(line) for line in lines if line.strip()]
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


def check_environment(repository):
    """The environment exists, admits only main and has no administrator bypass.

    A job that names a missing environment would create it with no branch rule."""
    try:
        environment = json.loads(run("gh", "api", f"repos/{repository}/environments/{ENVIRONMENT}"))
        policies = json.loads(run("gh", "api", f"repos/{repository}/environments/{ENVIRONMENT}/deployment-branch-policies"))
    except subprocess.CalledProcessError:
        raise Refused(f"the environment {ENVIRONMENT} is missing or unreadable; see {CHECKLIST}") from None
    rule = environment.get("deployment_branch_policy") or {}
    names = [(policy.get("name"), policy.get("type")) for policy in policies.get("branch_policies", [])]
    if environment.get("can_admins_bypass") is not False or rule != {
            "protected_branches": False, "custom_branch_policies": True} or names != [("main", "branch")]:
        raise Refused(f"the environment {ENVIRONMENT} must admit only the branch main, with no administrator "
                      f"bypass; see {CHECKLIST}")


def verify(args):
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
            "owner_tag": owner["crate_owner_tag"], "pending": [], "digests": {}}
    if not owner["crate_owner"]:
        return plan
    release = json.loads(run("gh", "api", f"repos/{repository}/releases/tags/{args.tag}"))
    if release.get("draft") or release.get("prerelease") or release.get("immutable") is not True:
        raise Refused(f"{args.tag} is not a published, immutable, stable release")
    assets = Path(args.work) / "assets"
    assets.mkdir(parents=True)
    names = [f"{name}-{version}.crate" for name in CRATES]
    run("gh", "release", "download", args.tag, "-R", repository, "-D", str(assets), "-p", "SHA256SUMS",
        *[option for name in names for option in ("-p", name)])
    sums = {}
    for line in (assets / "SHA256SUMS").read_text().splitlines():
        digest, file = line.split(maxsplit=1)
        sums[file] = digest
    recorded = {asset["name"]: asset.get("digest") for asset in release.get("assets", [])}
    identity = f"https://github.com/{repository}/.github/workflows/release.yml@refs/heads/main"
    for name, file in zip(CRATES, names):
        digest = sha256(assets / file)
        if sums.get(file) != digest or recorded.get(file) != f"sha256:{digest}":
            raise Refused(f"{file} does not match SHA256SUMS and the release's asset digest")
        run("gh", "attestation", "verify", str(assets / file), "--repo", repository, "--cert-identity", identity,
            "--source-ref", "refs/heads/main", "--source-digest", revision, "--deny-self-hosted-runners")
        plan["digests"][name] = digest
    # What `cargo publish` sends is packaged again from the same tree with the
    # same Cargo, so these bytes must equal the attested ones.
    target = Path(args.work) / "target"
    run("cargo", "package", "--locked", "--no-verify", *[option for name in CRATES for option in ("-p", name)],
        cwd=args.tree, env={**os.environ, "CARGO_TARGET_DIR": str(target)})
    for name, file in zip(CRATES, names):
        if sha256(target / "package" / file) != plan["digests"][name]:
            raise Refused(f"{file} packaged from {args.tag} differs from the attested release asset")
    states = {name: registry(name, version) for name in CRATES}
    plan["pending"] = decide(version, plan["digests"], states, args.mode)
    if plan["pending"] and args.operation == "publish":
        check_environment(repository)
    return plan


def confirm(plan, attempts=24, delay=10):
    for attempt in range(attempts):
        states = {name: registry(name, plan["version"]) for name in CRATES}
        if all(state and state.get("cksum") == plan["digests"][name] for name, state in states.items()):
            return f"crates.io lists {', '.join(CRATES)} at {plan['version']} with the attested checksums"
        for name, state in states.items():
            if state and state.get("cksum") != plan["digests"][name]:
                raise Refused(f"crates.io has {name} {plan['version']} with other bytes than the attested archive")
        if attempt + 1 < attempts:
            time.sleep(delay)
    raise Refused(f"crates.io's index does not list all three crates at {plan['version']} yet; dispatch again later")


def summary(plan):
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
                    output.write(f"pending={' '.join(plan['pending'])}\n")
        else:
            message = confirm(json.loads(Path(args.plan).read_text()))
    except (Refused, subprocess.CalledProcessError) as error:
        detail = getattr(error, "stderr", None) or error
        print(f"::error::{str(detail).strip()}", file=sys.stderr)
        return 1
    print(f"::notice::{message}")
    if "GITHUB_STEP_SUMMARY" in os.environ:
        with open(os.environ["GITHUB_STEP_SUMMARY"], "a") as out:
            out.write(f"{message}\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
