#!/usr/bin/env python3
"""Check all declared dependency edges, including target/build/dev dependencies.

This is a source policy, not a replacement for Cargo's resolver. Deliberately
small allowlists make a new capability or package an explicit review decision.
"""
import argparse
from pathlib import Path
import sys
import tomllib

RENDERER = {f"tilewright-{name}" for name in (
    "scene", "render-ui", "render-api", "render-resources", "render-world",
    "render-testkit", "render-wgpu", "render-sdl3")}
ALLOWED = {
    "tilewright-ui": {"unicode-segmentation"},
    "tilewright-app": {"tilewright-ui"},
    "tilewright-ui-render": {"tilewright-ui"} | RENDERER,
    "tilewright-ui-ffi": {"tilewright-ui", "tilewright-app", "tilewright-ui-render"},
    "tilewright-ui-web": {"tilewright-ui", "tilewright-app", "wasm-bindgen", "wasm-bindgen-futures", "js-sys", "web-sys", "serde_json"} | RENDERER,
    "tilewright-ui-native-demo": {"tilewright-ui", "tilewright-app", "tilewright-ui-render", "sdl3", "sha2"} | RENDERER,
}


def tables(manifest):
    for section in ("dependencies", "dev-dependencies", "build-dependencies"):
        yield from manifest.get(section, {}).items()
    for target in manifest.get("target", {}).values():
        yield from tables(target)


def check(root):
    errors = []
    workspace = tomllib.loads((root / "Cargo.toml").read_text())
    inherited = workspace.get("workspace", {}).get("dependencies", {})
    if "patch" in workspace or "replace" in workspace:
        errors.append("Cargo.toml: committed dependency overrides are forbidden")
    for alias, entry in inherited.items():
        entry = {"version": entry} if isinstance(entry, str) else entry
        if entry.get("package", alias) in RENDERER and entry.get("version") != "=0.6.1":
            errors.append(f"Cargo.toml: renderer dependency {alias} must use =0.6.1")
        if any(key in entry for key in ("git", "branch", "rev", "tag", "registry")):
            errors.append(f"Cargo.toml: workspace dependency {alias} has an unreviewed source")
        if "path" in entry:
            local = (root / entry["path"]).resolve()
            if not local.is_relative_to(root) or not (local / "Cargo.toml").is_file():
                errors.append(f"Cargo.toml: workspace dependency {alias} escapes or misses a local package")
        elif not entry.get("version", "").startswith("="):
            errors.append(f"Cargo.toml: workspace dependency {alias} must use an exact version")
    for config in (root / ".cargo").glob("config*"):
        errors.append(f"{config.relative_to(root)}: committed Cargo config requires explicit policy review")
    manifests = sorted(root.glob("crates/*/Cargo.toml"))
    manifests += sorted(root.glob("adapters/*/Cargo.toml"))
    manifests += sorted(root.glob("examples/*/Cargo.toml"))
    discovered = {path.parent.resolve() for path in manifests}
    for pattern in workspace.get("workspace", {}).get("members", []):
        for member in root.glob(pattern):
            if member.resolve() not in discovered:
                errors.append(f"Cargo.toml: workspace member {pattern} is outside the reviewed package layout")
    packages = {}
    for path in manifests:
        data = tomllib.loads(path.read_text())
        name = data["package"]["name"]
        if name in packages:
            errors.append(f"duplicate package {name}")
        packages[name] = path.parent.resolve()
    for path in manifests:
        data = tomllib.loads(path.read_text())
        name = data["package"]["name"]
        prefix = str(path.relative_to(root))
        if name not in ALLOWED:
            errors.append(f"{prefix}: package {name} has no reviewed dependency policy")
            continue
        if data["package"].get("publish") is not False:
            errors.append(f"{prefix}: prototype packages must set publish = false")
        if "patch" in data or "replace" in data:
            errors.append(f"{prefix}: dependency overrides are forbidden")
        for alias, spec in tables(data):
            spec = {"version": spec} if isinstance(spec, str) else dict(spec)
            if spec.get("workspace"):
                base = inherited.get(alias)
                if base is None:
                    errors.append(f"{prefix}: missing workspace dependency {alias}")
                    continue
                base = {"version": base} if isinstance(base, str) else base
                spec = {**base, **spec}
                base_dir = root
            else:
                base_dir = path.parent
            dependency = spec.get("package", alias)
            if dependency not in ALLOWED[name]:
                errors.append(f"{prefix}: forbidden edge {name} -> {dependency}")
            if any(key in spec for key in ("git", "branch", "rev", "tag", "registry")):
                errors.append(f"{prefix}: {alias} must use the reviewed crates.io or local workspace source")
            if dependency in packages:
                actual = (base_dir / spec.get("path", "")).resolve()
                if "path" not in spec or actual != packages[dependency]:
                    errors.append(f"{prefix}: {alias} must resolve to the local workspace package")
            else:
                if "path" in spec:
                    errors.append(f"{prefix}: external path dependency {alias} is forbidden")
                version = spec.get("version", "")
                if not version.startswith("=") or any(c in version for c in "*<>^~, "):
                    errors.append(f"{prefix}: external dependency {alias} needs one exact version")
                if dependency in RENDERER and version != "=0.6.1":
                    errors.append(f"{prefix}: renderer dependencies must retain =0.6.1")
    for name in ("tilewright-ui", "tilewright-app", "tilewright-ui-render"):
        if name not in packages:
            errors.append(f"missing required package {name}")
            continue
        lib = packages[name] / "src/lib.rs"
        if not lib.is_file() or "#![forbid(unsafe_code)]" not in lib.read_text():
            errors.append(f"{name}: crate root must forbid unsafe code")
    if "tilewright-ui-ffi" not in packages:
        errors.append("missing required package tilewright-ui-ffi")
    return errors


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    args = parser.parse_args()
    errors = check(args.root.resolve())
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print("Dependency boundaries, exact sources, unpublished packages and unsafe policy passed.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
