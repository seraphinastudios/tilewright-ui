#!/usr/bin/env python3
"""Release decisions for tilewright-ui, from git alone (docs/RELEASING.md).

plan           whether the run's commit is released, at which source version,
               and whether that release owns the workspace crate version
owner          whether an existing release tag owns its crate version
check-version  the crate version stays or moves one step from a base commit
check-crates   the packaged .crate files are the three crates, at the
               workspace version, from a clean checkout of the revision

Standard library only, so the release jobs run nothing but this repository's
code before they build.
"""
import argparse
import json
from pathlib import Path
import re
import subprocess
import sys
import tarfile
import tomllib

# Published in this order: a crate only after the crates it depends on.
CRATES = ("tilewright-ui", "tilewright-app", "tilewright-ui-render")
FIRST_VERSION = (0, 1, 0)
VERSION = re.compile(r"(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)")
TAG = re.compile(r"v" + VERSION.pattern)
# A squash-merged pull request's commit title is its Conventional Commits
# title; on main every typed commit releases, as in World.
TYPED = re.compile(r"[a-z][a-z0-9-]*(\([^)\n]*\))?!?: \S")
BREAKING = re.compile(r"^\s*[*|]?\s*BREAKING[ -]CHANGE:", re.IGNORECASE | re.MULTILINE)


class Refused(Exception):
    pass


def git(*args, root="."):
    return subprocess.run(("git", "-C", str(root)) + args, check=True,
                          capture_output=True, text=True).stdout


def parse(version, pattern=VERSION):
    match = pattern.fullmatch(version or "")
    if not match:
        raise Refused(f"not a release version: {version!r}")
    return tuple(int(part) for part in match.groups())


def text(version):
    return ".".join(map(str, version))


def crate_version(revision, root="."):
    """The workspace crate version at a revision."""
    manifest = tomllib.loads(git("show", f"{revision}:Cargo.toml", root=root))
    return parse(manifest["workspace"]["package"]["version"])


def previous_tag(revision, root=".", below=None):
    """The highest release tag the revision contains, below `below` if given."""
    tags = [tag for tag in git("tag", "--merged", revision, "--list", "v[0-9]*", root=root).split()
            if TAG.fullmatch(tag)]
    tags = sorted((parse(tag, TAG), tag) for tag in tags)
    if below is not None:
        tags = [entry for entry in tags if entry[0] < below]
    return tags[-1][1] if tags else None


def releases(message):
    """Whether a commit message calls for a release on main."""
    lines = message.splitlines() or [""]
    return bool(TYPED.match(lines[0]) or lines[0].startswith("Revert ") or BREAKING.search(message))


def ownership(revision, tag, previous, root="."):
    """A release owns its crate version when it is the first release at it."""
    version = crate_version(revision, root)
    if previous is None:
        return {"crate_version": text(version), "crate_owner": True, "crate_owner_tag": tag}
    before = crate_version(previous, root)
    if version < before:
        raise Refused(f"the crate version {text(version)} is lower than {previous}'s {text(before)}")
    if version > before:
        return {"crate_version": text(version), "crate_owner": True, "crate_owner_tag": tag}
    # The same version: the owner is the earliest release at it.
    owner = previous
    while (earlier := previous_tag(owner, root, below=parse(owner, TAG))) and \
            crate_version(earlier, root) == version:
        owner = earlier
    return {"crate_version": text(version), "crate_owner": False, "crate_owner_tag": owner}


# What reaches a crate's users: each crate's packaged files (not its tests)
# and what Cargo packages from the workspace.
PACKAGED = ("Cargo.toml", "Cargo.lock", "LICENSE", *(f"crates/{name}" for name in CRATES),
            *(f":(exclude)crates/{name}/tests" for name in CRATES))


def crate_changed(owner_tag, revision, root="."):
    """Whether packaged content changed since the release that owns the crate version."""
    return bool(git("diff", "--name-only", owner_tag, revision, "--", *PACKAGED, root=root).strip())


def plan(revision, event, root="."):
    previous = previous_tag(revision, root)
    result = {"revision": git("rev-parse", revision, root=root).strip(), "previous_tag": previous}
    if previous is None:
        if event != "workflow_dispatch":
            return {**result, "release": False,
                    "reason": "No release exists yet. A merge never cuts the first release: "
                              "run the Release workflow on main (docs/CRATE-FIRST-PUBLICATION.md)."}
        version, reason = FIRST_VERSION, "The first release, started by hand."
    else:
        log = git("log", "--format=%B%x00", f"{previous}..{revision}", root=root)
        messages = [message.strip("\n") for message in log.split("\0")[:-1]]
        if not any(releases(message) for message in messages):
            return {**result, "release": False,
                    "reason": f"No commit since {previous} calls for a release."}
        major, minor, _ = parse(previous, TAG)
        version = (major, minor + 1, 0)
        reason = f"{len(messages)} commit(s) since {previous}."
    tag = "v" + text(version)
    owner = ownership(revision, tag, previous, root)
    changed = not owner["crate_owner"] and crate_changed(owner["crate_owner_tag"], revision, root)
    return {**result, "release": True, "version": text(version), "tag": tag, "reason": reason,
            **owner, "crate_changed": changed}


def check_version(base, root="."):
    old, new = crate_version(base, root), crate_version("HEAD", root)
    allowed = {old, (old[0], old[1], old[2] + 1), (old[0], old[1] + 1, 0)}
    if new not in allowed:
        raise Refused(f"the crate version must stay {text(old)} or move to the next patch or "
                      f"minor version, not {text(new)}")
    if new >= (1, 0, 0):
        raise Refused("the crates stay below 1.0.0 until a reviewed change says otherwise")
    return f"crate version {text(old)} -> {text(new)}"


def check_crates(directory, revision, root="."):
    """Each packaged crate holds its sources, README.md and LICENSE, from a clean tree."""
    version = text(crate_version(revision, root))
    sha = git("rev-parse", revision, root=root).strip()
    found = sorted(path.name for path in Path(directory).glob("*.crate"))
    expected = sorted(f"{name}-{version}.crate" for name in CRATES)
    if found != expected:
        raise Refused(f"expected exactly {expected} in {directory}, found {found}")
    for name in CRATES:
        prefix = f"{name}-{version}/"
        with tarfile.open(Path(directory) / f"{name}-{version}.crate") as archive:
            members = {member.name: member for member in archive.getmembers()}
            for required in ("Cargo.toml", "README.md", "LICENSE", "src/lib.rs", ".cargo_vcs_info.json"):
                if prefix + required not in members:
                    raise Refused(f"{name} {version} lacks {required}")
            if any(not member.startswith(prefix) or not (members[member].isfile() or members[member].isdir())
                   for member in members):
                raise Refused(f"{name} {version} holds an entry outside {prefix} or a link")
            vcs = json.load(archive.extractfile(prefix + ".cargo_vcs_info.json"))
            if vcs.get("git", {}).get("sha1") != sha or vcs["git"].get("dirty"):
                raise Refused(f"{name} {version} was not packaged from a clean checkout of {sha}")
            packaged = tomllib.loads(archive.extractfile(prefix + "Cargo.toml").read().decode())
            if packaged["package"]["name"] != name or packaged["package"]["version"] != version:
                raise Refused(f"{name}-{version}.crate holds another package")
    return f"{', '.join(expected)} are the three crates at {version} from {sha}"


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--root", default=".")
    commands = parser.add_subparsers(dest="command", required=True)
    command = commands.add_parser("plan")
    command.add_argument("--revision", default="HEAD")
    command.add_argument("--event", required=True)
    command = commands.add_parser("owner")
    command.add_argument("--tag", required=True)
    command = commands.add_parser("check-version")
    command.add_argument("--base", required=True)
    command = commands.add_parser("check-crates")
    command.add_argument("--dir", required=True)
    command.add_argument("--revision", default="HEAD")
    args = parser.parse_args(argv)
    try:
        if args.command == "plan":
            print(json.dumps(plan(args.revision, args.event, args.root), sort_keys=True))
        elif args.command == "owner":
            revision = git("rev-parse", f"{args.tag}^{{commit}}", root=args.root).strip()
            previous = previous_tag(revision, args.root, below=parse(args.tag, TAG))
            print(json.dumps({"revision": revision, **ownership(revision, args.tag, previous, args.root)},
                             sort_keys=True))
        elif args.command == "check-version":
            print(check_version(args.base, args.root))
        else:
            print(check_crates(args.dir, args.revision, args.root))
    except (Refused, subprocess.CalledProcessError, KeyError, tomllib.TOMLDecodeError) as error:
        detail = getattr(error, "stderr", None) or error
        print(f"error: {detail}".rstrip(), file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
