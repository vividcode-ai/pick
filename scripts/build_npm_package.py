#!/usr/bin/env python3
"""Stage and package the @vividcodeai/pick npm module."""

import argparse
import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parent
REPO_ROOT = SCRIPT_DIR.parent
PICK_ROOT = REPO_ROOT / "npm" / "pick"
PICK_NPM_NAME = "@vividcodeai/pick"

PICK_PLATFORM_PACKAGES: dict[str, dict[str, str]] = {
    "pick-linux-x64": {
        "npm_name": "@vividcodeai/pick-linux-x64",
        "npm_tag": "linux-x64",
        "target_triple": "x86_64-unknown-linux-gnu",
        "os": "linux",
        "cpu": "x64",
    },
    "pick-linux-arm64": {
        "npm_name": "@vividcodeai/pick-linux-arm64",
        "npm_tag": "linux-arm64",
        "target_triple": "aarch64-unknown-linux-gnu",
        "os": "linux",
        "cpu": "arm64",
    },
    "pick-darwin-x64": {
        "npm_name": "@vividcodeai/pick-darwin-x64",
        "npm_tag": "darwin-x64",
        "target_triple": "x86_64-apple-darwin",
        "os": "darwin",
        "cpu": "x64",
    },
    "pick-darwin-arm64": {
        "npm_name": "@vividcodeai/pick-darwin-arm64",
        "npm_tag": "darwin-arm64",
        "target_triple": "aarch64-apple-darwin",
        "os": "darwin",
        "cpu": "arm64",
    },
    "pick-win32-x64": {
        "npm_name": "@vividcodeai/pick-win32-x64",
        "npm_tag": "win32-x64",
        "target_triple": "x86_64-pc-windows-msvc",
        "os": "win32",
        "cpu": "x64",
    },
    "pick-win32-arm64": {
        "npm_name": "@vividcodeai/pick-win32-arm64",
        "npm_tag": "win32-arm64",
        "target_triple": "aarch64-pc-windows-msvc",
        "os": "win32",
        "cpu": "arm64",
    },
}

PACKAGE_EXPANSIONS: dict[str, list[str]] = {
    "pick": ["pick", *PICK_PLATFORM_PACKAGES],
}

PACKAGE_NATIVE_COMPONENTS: dict[str, list[str]] = {
    "pick": [],
    **{name: ["pick"] for name in PICK_PLATFORM_PACKAGES},
}

PACKAGE_CHOICES = tuple(PACKAGE_NATIVE_COMPONENTS)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Build the Pick CLI npm package.")
    parser.add_argument(
        "--package",
        choices=PACKAGE_CHOICES,
        default="pick",
        help="Which npm package to stage (default: pick).",
    )
    parser.add_argument(
        "--version",
        help="Version number to write to package.json inside the staged package.",
    )
    parser.add_argument(
        "--release-version",
        help="Version to stage for npm release.",
    )
    parser.add_argument(
        "--staging-dir",
        type=Path,
        help="Directory to stage the package contents.",
    )
    parser.add_argument(
        "--pack-output",
        type=Path,
        help="Path where the generated npm tarball should be written.",
    )
    parser.add_argument(
        "--binary-src",
        type=Path,
        help="Directory containing the native binary to bundle.",
    )
    return parser.parse_args()


def compute_platform_package_version(version: str, platform_tag: str) -> str:
    return f"{version}-{platform_tag}"


def main() -> int:
    args = parse_args()

    package = args.package
    version = args.version
    release_version = args.release_version
    if release_version:
        if version and version != release_version:
            raise RuntimeError("--version and --release-version must match when both are provided.")
        version = release_version

    if not version:
        raise RuntimeError("Must specify --version or --release-version.")

    staging_dir = args.staging_dir.resolve() if args.staging_dir else Path(tempfile.mkdtemp(prefix="pick-npm-stage-"))
    staging_dir.mkdir(parents=True, exist_ok=True)

    try:
        stage_sources(staging_dir, version, package, args.binary_src)

        if args.pack_output is not None:
            output_path = run_npm_pack(staging_dir, args.pack_output)
            print(f"npm pack output written to {output_path}")
    finally:
        if args.staging_dir is None:
            pass

    return 0


def stage_sources(staging_dir: Path, version: str, package: str, binary_src: Path | None) -> None:
    package_json: dict

    if package == "pick":
        bin_dir = staging_dir / "bin"
        bin_dir.mkdir(parents=True, exist_ok=True)
        shutil.copy2(PICK_ROOT / "bin" / "pick.js", bin_dir / "pick.js")

        readme_src = REPO_ROOT / "README.md"
        if readme_src.exists():
            shutil.copy2(readme_src, staging_dir / "README.md")

        package_json_path = PICK_ROOT / "package.json"
        with open(package_json_path, "r", encoding="utf-8") as fh:
            package_json = json.load(fh)
        package_json["version"] = version
        package_json["files"] = ["bin/pick.js"]
        package_json["optionalDependencies"] = {
            PICK_PLATFORM_PACKAGES[pp]["npm_name"]: (
                f"npm:{PICK_NPM_NAME}@"
                f"{compute_platform_package_version(version, PICK_PLATFORM_PACKAGES[pp]['npm_tag'])}"
            )
            for pp in PACKAGE_EXPANSIONS["pick"]
            if pp != "pick"
        }

    elif package in PICK_PLATFORM_PACKAGES:
        platform_package = PICK_PLATFORM_PACKAGES[package]
        platform_tag = platform_package["npm_tag"]
        platform_version = compute_platform_package_version(version, platform_tag)

        package_json = {
            "name": PICK_NPM_NAME,
            "version": platform_version,
            "license": "MIT",
            "os": [platform_package["os"]],
            "cpu": [platform_package["cpu"]],
            "files": ["vendor"],
            "repository": {
                "type": "git",
                "url": "git+https://github.com/vividcodeai/pick.git",
            },
        }

        if binary_src is not None:
            binary_src = binary_src.resolve()
            vendor_dest = staging_dir / "vendor"
            vendor_dest.mkdir(parents=True, exist_ok=True)

            binary_name = "pick.exe" if "win32" in package else "pick"
            src_binary = binary_src / binary_name
            if not src_binary.exists():
                raise RuntimeError(f"Binary not found: {src_binary}")
            shutil.copy2(src_binary, vendor_dest / binary_name)
            if "win32" not in package:
                (vendor_dest / binary_name).chmod(0o755)
        else:
            raise RuntimeError(f"Platform package '{package}' requires --binary-src.")

        readme_src = REPO_ROOT / "README.md"
        if readme_src.exists():
            shutil.copy2(readme_src, staging_dir / "README.md")

    else:
        raise RuntimeError(f"Unknown package '{package}'.")

    with open(staging_dir / "package.json", "w", encoding="utf-8") as out:
        json.dump(package_json, out, indent=2)
        out.write("\n")


def run_npm_pack(staging_dir: Path, output_path: Path) -> Path:
    output_path = output_path.resolve()
    output_path.parent.mkdir(parents=True, exist_ok=True)

    with tempfile.TemporaryDirectory(prefix="pick-npm-pack-") as pack_dir_str:
        pack_dir = Path(pack_dir_str)
        npm_cache_dir = pack_dir / "npm-cache"
        npm_logs_dir = pack_dir / "npm-logs"
        npm_cache_dir.mkdir()
        npm_logs_dir.mkdir()
        env = os.environ.copy()
        env["NPM_CONFIG_CACHE"] = str(npm_cache_dir)
        env["NPM_CONFIG_LOGS_DIR"] = str(npm_logs_dir)
        stdout = subprocess.check_output(
            ["npm", "pack", "--json", "--pack-destination", str(pack_dir)],
            cwd=staging_dir,
            env=env,
            text=True,
        )
        try:
            pack_output = json.loads(stdout)
        except json.JSONDecodeError as exc:
            raise RuntimeError("Failed to parse npm pack output.") from exc

        if not pack_output:
            raise RuntimeError("npm pack did not produce an output tarball.")

        tarball_name = pack_output[0].get("filename") or pack_output[0].get("name")
        if not tarball_name:
            raise RuntimeError("Unable to determine npm pack output filename.")

        tarball_path = pack_dir / tarball_name
        if not tarball_path.exists():
            raise RuntimeError(f"Expected npm pack output not found: {tarball_path}")

        shutil.move(str(tarball_path), output_path)

    return output_path


if __name__ == "__main__":
    sys.exit(main())
