#!/usr/bin/env node
import { createRequire } from "module";
import { spawn } from "child_process";
import { existsSync } from "fs";
import { join, dirname } from "path";
import { fileURLToPath } from "url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const require = createRequire(import.meta.url);

const PLATFORM_PACKAGES = {
  "linux-x64": "@vividcodeai/pick-linux-x64",
  "linux-arm64": "@vividcodeai/pick-linux-arm64",
  "darwin-x64": "@vividcodeai/pick-darwin-x64",
  "darwin-arm64": "@vividcodeai/pick-darwin-arm64",
  "win32-x64": "@vividcodeai/pick-win32-x64",
  "win32-arm64": "@vividcodeai/pick-win32-arm64",
};

const BINARY_NAME = process.platform === "win32" ? "pick.exe" : "pick";

function getTargetTriple(platform, arch) {
  if (platform === "linux" && arch === "x64") return "x86_64-unknown-linux-gnu";
  if (platform === "linux" && arch === "arm64") return "aarch64-unknown-linux-gnu";
  if (platform === "darwin" && arch === "x64") return "x86_64-apple-darwin";
  if (platform === "darwin" && arch === "arm64") return "aarch64-apple-darwin";
  if (platform === "win32" && arch === "x64") return "x86_64-pc-windows-msvc";
  if (platform === "win32" && arch === "arm64") return "aarch64-pc-windows-msvc";
  return null;
}

function main() {
  const platform = process.platform;
  const arch = process.arch;
  const mapKey = `${platform}-${arch}`;
  const pkgName = PLATFORM_PACKAGES[mapKey];

  if (!pkgName) {
    console.error(`Unsupported platform: ${platform} ${arch}`);
    process.exit(1);
  }

  let pkgPath;
  try {
    pkgPath = dirname(require.resolve(`${pkgName}/package.json`));
  } catch {
    console.error(
      `Platform package ${pkgName} not installed. Try: npm install -g ${pkgName}`
    );
    process.exit(1);
  }

  const binaryPath = join(pkgPath, "vendor", BINARY_NAME);
  if (!existsSync(binaryPath)) {
    console.error(`Binary not found at ${binaryPath}`);
    process.exit(1);
  }

  const child = spawn(binaryPath, process.argv.slice(2), {
    stdio: "inherit",
    env: { ...process.env, PICK_MANAGED_BY_NPM: "1" },
  });

  child.on("exit", (code) => process.exit(code));
}

main();
