#!/usr/bin/env python3
"""Stage all Pick npm packages for release."""

import argparse
import json
import os
import shutil
import subprocess
import tempfile
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
BUILD_SCRIPT = REPO_ROOT / "scripts" / "build_npm_package.py"

PICK_PLATFORM_PACKAGES: dict[str, dict[str, str]] = {
    "pick-linux-x64": {
        "npm_name": "@vividcodeai/pick-linux-x64",
        "npm_tag": "linux-x64",
        "build_target": "linux-x86_64",
        "target_triple": "x86_64-unknown-linux-gnu",
        "os": "linux",
        "cpu": "x64",
        "binary_name": "pick",
    },
    "pick-linux-arm64": {
        "npm_name": "@vividcodeai/pick-linux-arm64",
        "npm_tag": "linux-arm64",
        "build_target": "linux-aarch64",
        "target_triple": "aarch64-unknown-linux-gnu",
        "os": "linux",
        "cpu": "arm64",
        "binary_name": "pick",
    },
    "pick-darwin-x64": {
        "npm_name": "@vividcodeai/pick-darwin-x64",
        "npm_tag": "darwin-x64",
        "build_target": "macos-x86_64",
        "target_triple": "x86_64-apple-darwin",
        "os": "darwin",
        "cpu": "x64",
        "binary_name": "pick",
    },
    "pick-darwin-arm64": {
        "npm_name": "@vividcodeai/pick-darwin-arm64",
        "npm_tag": "darwin-arm64",
        "build_target": "macos-aarch64",
        "target_triple": "aarch64-apple-darwin",
        "os": "darwin",
        "cpu": "arm64",
        "binary_name": "pick",
    },
    "pick-win32-x64": {
        "npm_name": "@vividcodeai/pick-win32-x64",
        "npm_tag": "win32-x64",
        "build_target": "windows-x86_64",
        "target_triple": "x86_64-pc-windows-msvc",
        "os": "win32",
        "cpu": "x64",
        "binary_name": "pick.exe",
    },
    "pick-win32-arm64": {
        "npm_name": "@vividcodeai/pick-win32-arm64",
        "npm_tag": "win32-arm64",
        "build_target": "windows-aarch64",
        "target_triple": "aarch64-pc-windows-msvc",
        "os": "win32",
        "cpu": "arm64",
        "binary_name": "pick.exe",
    },
}


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Stage all Pick npm packages for release.")
    parser.add_argument(
        "--release-version",
        required=True,
        help="Version to stage (e.g. 0.1.0 or 0.1.0-alpha.1).",
    )
    parser.add_argument(
        "--binary-dir",
        type=Path,
        required=True,
        help="Directory containing compiled native binaries.",
    )
    parser.add_argument(
        "--output-dir",
        type=Path,
        default=None,
        help="Directory where npm tarballs should be written (default: dist/npm).",
    )
    return parser.parse_args()


def tarball_name_for_package(package: str, version: str) -> str:
    if package in PICK_PLATFORM_PACKAGES:
        platform = package.removeprefix("pick-")
        return f"pick-npm-{platform}-{version}.tgz"
    return f"pick-npm-{version}.tgz"


def run_command(cmd: list[str]) -> None:
    print("+", " ".join(cmd), flush=True)
    subprocess.run(cmd, cwd=REPO_ROOT, check=True)


def main() -> int:
    args = parse_args()

    output_dir = args.output_dir or (REPO_ROOT / "dist" / "npm")
    output_dir.mkdir(parents=True, exist_ok=True)

    binary_dir = args.binary_dir.resolve()
    if not binary_dir.exists():
        raise RuntimeError(f"Binary directory not found: {binary_dir}")

    runner_temp = Path(os.environ.get("RUNNER_TEMP", tempfile.gettempdir()))

    all_packages = ["pick"] + list(PICK_PLATFORM_PACKAGES)

    for package in all_packages:
        staging_dir = Path(
            tempfile.mkdtemp(prefix=f"pick-npm-stage-{package}-", dir=runner_temp)
        )
        pack_output = output_dir / tarball_name_for_package(package, args.release_version)

        print(f"Staging {package} in {staging_dir}", flush=True)

        cmd = [
            str(BUILD_SCRIPT),
            "--package", package,
            "--release-version", args.release_version,
            "--staging-dir", str(staging_dir),
            "--pack-output", str(pack_output),
        ]

        if package in PICK_PLATFORM_PACKAGES:
            pkg_info = PICK_PLATFORM_PACKAGES[package]
            target = pkg_info["build_target"]
            binary_path = binary_dir / target / pkg_info["binary_name"]
            if not binary_path.exists():
                print(f"  Binary not found at {binary_path}, trying target/release...", flush=True)
                fallback = REPO_ROOT / "target" / "release" / pkg_info["binary_name"]
                if fallback.exists():
                    binary_path = fallback

            if not binary_path.exists():
                print(f"  Skipping {package}: binary not found at {binary_path}", flush=True)
                shutil.rmtree(staging_dir, ignore_errors=True)
                continue

            binary_src_dir = binary_path.parent
            cmd.extend(["--binary-src", str(binary_src_dir)])

        try:
            run_command(cmd)
        finally:
            shutil.rmtree(staging_dir, ignore_errors=True)

        print(f"  Staged {package} at {pack_output}", flush=True)

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
