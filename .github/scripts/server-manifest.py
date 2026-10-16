#!/usr/bin/env python3
"""Build server-manifest.json, the manifest the desktop client reads to fetch and verify vela-server.

Input is a directory holding, for each platform key, the server binary, its zip, and a `.sig` beside each
(the verbatim text `tauri signer sign` wrote). The output maps each platform key to url/sha256/signature for
the binary and for the zip.
"""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import pathlib
import sys

PLATKEYS = ("linux-x86_64", "linux-aarch64", "darwin-aarch64", "darwin-x86_64")

_spec = importlib.util.spec_from_file_location(
    "updater_manifest", pathlib.Path(__file__).resolve().parent / "updater-manifest.py"
)
_updater = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(_updater)
asset_url = _updater.asset_url
read_signature = _updater.read_signature


def require_file(directory: pathlib.Path, name: str, key: str) -> pathlib.Path:
    path = directory / name
    if not path.is_file():
        sys.exit(f"missing {name} for platform {key} under {directory}")
    return path


def sha256_hex(path: pathlib.Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def build_manifest(directory: pathlib.Path, repo: str, tag: str, version: str) -> dict:
    platforms = {}
    for key in PLATKEYS:
        binary = require_file(directory, f"vela-server-{key}", key)
        archive = require_file(directory, f"vela-server-{key}.zip", key)
        platforms[key] = {
            "url": asset_url(repo, tag, binary.name),
            "sha256": sha256_hex(binary),
            "signature": read_signature(binary),
            "zip_url": asset_url(repo, tag, archive.name),
            "zip_sha256": sha256_hex(archive),
            "zip_signature": read_signature(archive),
        }
    return {"version": version, "platforms": platforms}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dir", required=True, type=pathlib.Path, help="directory with the signed server artifacts")
    parser.add_argument("--repo", required=True, help="owner/name, normally GITHUB_REPOSITORY")
    parser.add_argument("--tag", required=True, help="the release tag, e.g. v0.2.15")
    parser.add_argument("--version", required=True, help="the version without the leading v")
    parser.add_argument("--out", required=True, type=pathlib.Path, help="where to write the manifest")
    args = parser.parse_args()

    if not args.dir.is_dir():
        sys.exit(f"{args.dir} is not a directory")
    manifest = build_manifest(args.dir, args.repo, args.tag, args.version)
    args.out.write_text(json.dumps(manifest, indent=2) + "\n")


if __name__ == "__main__":
    main()
