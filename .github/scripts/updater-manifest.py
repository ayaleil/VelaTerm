#!/usr/bin/env python3
"""Build the updater manifests the two editions of VelaTerm fetch.

Upstream serves these from an API that reads a query string. A fork publishes release assets instead, so the
manifests are generated here and uploaded next to the artifacts they describe.

Two shapes, because the two editions have different updaters:

  linux  The Electron edition on Linux. electron/main.cjs validates {version, url, signature} as strings and
         requires an https url, so the architecture is part of the file name rather than a query parameter.
  tauri  The Tauri edition on Windows and macOS. tauri-plugin-updater reads the static-JSON shape, keyed by
         "<os>-<arch>", and verifies each signature against plugins.updater.pubkey.

Every signature is the verbatim text of the `.sig` that `tauri signer sign` wrote beside the artifact.
"""

from __future__ import annotations

import argparse
import json
import pathlib
import sys

# Tauri's platform keys, matched against an artifact's file name. The updater asks for the key built from the
# running target, so a missing entry means that platform silently never updates - hence the check below.
# Windows ships min and full installers but a key holds one, so updates use full (see release.yml).
TAURI_TARGETS = (
    ("-full-setup.exe", "windows-x86_64"),
    (".app.tar.gz", "darwin-aarch64"),
)


def read_signature(artifact: pathlib.Path) -> str:
    signature = artifact.with_name(artifact.name + ".sig")
    if not signature.is_file():
        sys.exit(f"{artifact.name} has no signature at {signature.name}; was it built with signing enabled?")
    text = signature.read_text().strip()
    if not text:
        sys.exit(f"{signature.name} is empty")
    return text


def asset_url(repo: str, tag: str, name: str) -> str:
    return f"https://github.com/{repo}/releases/download/{tag}/{name}"


def linux_manifest(artifact: pathlib.Path, repo: str, tag: str) -> dict:
    return {
        "version": tag.lstrip("v"),
        "notes": f"VelaTerm {tag}",
        "url": asset_url(repo, tag, artifact.name),
        "signature": read_signature(artifact),
    }


def tauri_manifest(directory: pathlib.Path, repo: str, tag: str) -> dict:
    platforms = {}
    for artifact in sorted(directory.rglob("*")):
        if artifact.suffix == ".sig" or not artifact.is_file():
            continue
        for suffix, key in TAURI_TARGETS:
            if artifact.name.endswith(suffix):
                platforms[key] = {
                    "signature": read_signature(artifact),
                    "url": asset_url(repo, tag, artifact.name),
                }
    if not platforms:
        names = ", ".join(p.name for p in directory.rglob("*") if p.is_file()) or "nothing"
        sys.exit(f"no updater artifact matched {[s for s, _ in TAURI_TARGETS]} under {directory} (found: {names})")
    return {
        "version": tag.lstrip("v"),
        "notes": f"VelaTerm {tag}",
        "platforms": platforms,
    }


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("shape", choices=("linux", "tauri"))
    parser.add_argument("path", type=pathlib.Path, help="the AppImage, or the directory holding the bundles")
    parser.add_argument("--repo", required=True, help="owner/name, normally GITHUB_REPOSITORY")
    parser.add_argument("--tag", required=True, help="the release tag, normally GITHUB_REF_NAME")
    args = parser.parse_args()

    if args.shape == "linux":
        if not args.path.is_file():
            sys.exit(f"{args.path} is not a file")
        manifest = linux_manifest(args.path, args.repo, args.tag)
    else:
        if not args.path.is_dir():
            sys.exit(f"{args.path} is not a directory")
        manifest = tauri_manifest(args.path, args.repo, args.tag)
    print(json.dumps(manifest, indent=2))


if __name__ == "__main__":
    main()
