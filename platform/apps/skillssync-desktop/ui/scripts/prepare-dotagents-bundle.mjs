#!/usr/bin/env node

import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const DOTAGENTS_VERSION = "0.10.0";
const NODE_VERSION = "22.22.0";

const TARGETS = {
  "darwin-arm64": {
    nodePackage: "node-bin-darwin-arm64",
    nodeBinaryCandidates: ["node_modules/node-bin-darwin-arm64/bin/node"],
    launcherName: "dotagents",
  },
  "darwin-x64": {
    nodePackage: "node-darwin-x64",
    nodeBinaryCandidates: ["node_modules/node-darwin-x64/bin/node"],
    launcherName: "dotagents",
  },
  "linux-x64": {
    nodePackage: "node-linux-x64",
    nodeBinaryCandidates: ["node_modules/node-linux-x64/bin/node"],
    launcherName: "dotagents",
  },
  "linux-arm64": {
    nodePackage: "node-linux-arm64",
    nodeBinaryCandidates: ["node_modules/node-linux-arm64/bin/node"],
    launcherName: "dotagents",
  },
  "windows-x64": {
    nodePackage: "node-win-x64",
    nodeBinaryCandidates: ["node_modules/node-win-x64/bin/node.exe"],
    launcherName: "dotagents.cmd",
  },
};

const DOTAGENTS_CLI_PATH = "node_modules/@sentry/dotagents/dist/cli/index.js";

const __filename = fileURLToPath(import.meta.url);
const SCRIPT_DIR = path.dirname(__filename);
const UI_DIR = path.resolve(SCRIPT_DIR, "..");
const DESKTOP_DIR = path.resolve(UI_DIR, "..");
const PLATFORM_DIR = path.resolve(UI_DIR, "../../..");
const CACHE_ROOT = path.join(PLATFORM_DIR, "target", "dotagents-runtime-cache");
const DEV_RUNTIME_ROOT = path.join(
  PLATFORM_DIR,
  "target",
  "debug",
  "bin",
  "dotagents",
);
const TAURI_BUNDLE_ROOT = path.join(
  DESKTOP_DIR,
  "src-tauri",
  "bin",
  "dotagents",
);

function detectHostTarget() {
  const platform = os.platform();
  const arch = os.arch();

  if (platform === "darwin" && arch === "arm64") {
    return "darwin-arm64";
  }
  if (platform === "darwin" && arch === "x64") {
    return "darwin-x64";
  }
  if (platform === "linux" && arch === "x64") {
    return "linux-x64";
  }
  if (platform === "linux" && arch === "arm64") {
    return "linux-arm64";
  }
  if (platform === "win32" && arch === "x64") {
    return "windows-x64";
  }

  throw new Error(
    `Unsupported host for bundled dotagents runtime: ${platform}-${arch}`,
  );
}

function npmCommand() {
  return process.platform === "win32" ? "npm.cmd" : "npm";
}

function ensureDirectory(dirPath) {
  fs.mkdirSync(dirPath, { recursive: true });
}

function writeJson(filePath, value) {
  fs.writeFileSync(filePath, `${JSON.stringify(value, null, 2)}\n`, "utf8");
}

function parseJsonFile(filePath) {
  if (!fs.existsSync(filePath)) {
    return null;
  }
  try {
    return JSON.parse(fs.readFileSync(filePath, "utf8"));
  } catch {
    return null;
  }
}

function findExistingRelativePath(baseDir, relativePaths) {
  for (const relativePath of relativePaths) {
    const absolutePath = path.join(baseDir, relativePath);
    if (isUsableBinaryFile(absolutePath)) {
      return relativePath;
    }
  }
  return null;
}

function isUsableBinaryFile(filePath) {
  if (!fs.existsSync(filePath)) {
    return false;
  }
  try {
    const stat = fs.statSync(filePath);
    return stat.isFile() && stat.size > 0;
  } catch {
    return false;
  }
}

function resolveNodeBinaryRelativePath(baseDir, target) {
  const targetConfig = TARGETS[target];
  return findExistingRelativePath(baseDir, targetConfig.nodeBinaryCandidates);
}

function resolveDotagentsCliRelativePath(baseDir) {
  if (fs.existsSync(path.join(baseDir, DOTAGENTS_CLI_PATH))) {
    return DOTAGENTS_CLI_PATH;
  }
  return null;
}

function removeDirectoryIfExists(dirPath) {
  fs.rmSync(dirPath, { recursive: true, force: true });
}

function installRuntimeDependencies(cacheInstallDir, target) {
  ensureDirectory(cacheInstallDir);
  const targetConfig = TARGETS[target];

  const packageJsonPath = path.join(cacheInstallDir, "package.json");
  if (!fs.existsSync(packageJsonPath)) {
    writeJson(packageJsonPath, {
      name: "skillssync-dotagents-runtime-cache",
      private: true,
      version: "1.0.0",
      type: "module",
    });
  }

  execFileSync(
    npmCommand(),
    [
      "install",
      "--no-save",
      "--omit=dev",
      "--no-audit",
      "--no-fund",
      `@sentry/dotagents@${DOTAGENTS_VERSION}`,
      `${targetConfig.nodePackage}@${NODE_VERSION}`,
    ],
    {
      cwd: cacheInstallDir,
      stdio: "inherit",
      env: {
        ...process.env,
        npm_config_update_notifier: "false",
      },
    },
  );
}

function ensureCacheReady(cacheInstallDir, target) {
  const targetConfig = TARGETS[target];
  const markerPath = path.join(cacheInstallDir, ".prepared.json");
  const marker = parseJsonFile(markerPath);
  const nodeBinaryRelativePath = resolveNodeBinaryRelativePath(
    cacheInstallDir,
    target,
  );
  const dotagentsCliRelativePath =
    resolveDotagentsCliRelativePath(cacheInstallDir);

  const cacheReusable =
    marker &&
    marker.target === target &&
    marker.nodePackage === targetConfig.nodePackage &&
    marker.dotagentsVersion === DOTAGENTS_VERSION &&
    marker.nodeVersion === NODE_VERSION &&
    typeof nodeBinaryRelativePath === "string" &&
    typeof dotagentsCliRelativePath === "string";

  if (!cacheReusable) {
    removeDirectoryIfExists(cacheInstallDir);
    installRuntimeDependencies(cacheInstallDir, target);
  }

  const resolvedNodeBinary = resolveNodeBinaryRelativePath(
    cacheInstallDir,
    target,
  );
  const resolvedCli = resolveDotagentsCliRelativePath(cacheInstallDir);
  if (!resolvedNodeBinary || !resolvedCli) {
    throw new Error(
      `Failed to prepare dotagents cache for ${target}: required files missing after npm install`,
    );
  }
  if (!isUsableBinaryFile(path.join(cacheInstallDir, resolvedNodeBinary))) {
    throw new Error(
      `Failed to prepare dotagents cache for ${target}: selected node binary is empty or invalid (${resolvedNodeBinary})`,
    );
  }

  writeJson(markerPath, {
    target,
    nodePackage: targetConfig.nodePackage,
    dotagentsVersion: DOTAGENTS_VERSION,
    nodeVersion: NODE_VERSION,
    nodeBinaryRelativePath: resolvedNodeBinary,
    dotagentsCliRelativePath: resolvedCli,
  });

  return {
    nodeBinaryRelativePath: resolvedNodeBinary,
    dotagentsCliRelativePath: resolvedCli,
  };
}

function toPosixPath(input) {
  return input.split(path.sep).join("/");
}

function sha256OfFile(filePath) {
  const hasher = createHash("sha256");
  hasher.update(fs.readFileSync(filePath));
  return hasher.digest("hex");
}

function writeLauncher(
  targetDir,
  target,
  nodeBinaryRelativePath,
  dotagentsCliRelativePath,
) {
  const targetConfig = TARGETS[target];
  const launcherPath = path.join(targetDir, targetConfig.launcherName);
  const normalizedCandidates = targetConfig.nodeBinaryCandidates.map(
    (candidatePath) => toPosixPath(candidatePath),
  );
  const launcherCandidates = [
    toPosixPath(nodeBinaryRelativePath),
    ...normalizedCandidates.filter(
      (candidatePath) => candidatePath !== toPosixPath(nodeBinaryRelativePath),
    ),
  ];

  if (target === "windows-x64") {
    const windowsCandidates = launcherCandidates.map((candidatePath) =>
      candidatePath.split("/").join("\\"),
    );
    const cliPath = dotagentsCliRelativePath.split("/").join("\\");
    const launcherLines = [
      "@echo off",
      "setlocal",
      'set "SCRIPT_DIR=%~dp0"',
      `set "NODE_BIN=%SCRIPT_DIR%${windowsCandidates[0]}"`,
    ];
    for (const fallbackPath of windowsCandidates.slice(1)) {
      launcherLines.push(
        `if not exist "%NODE_BIN%" set "NODE_BIN=%SCRIPT_DIR%${fallbackPath}"`,
      );
      launcherLines.push(
        `for %%I in ("%NODE_BIN%") do if %%~zI EQU 0 set "NODE_BIN=%SCRIPT_DIR%${fallbackPath}"`,
      );
    }
    launcherLines.push(
      'if not exist "%NODE_BIN%" (',
      "  echo dotagents bundled node runtime missing: %NODE_BIN% 1>&2",
      "  exit /b 127",
      ")",
      'for %%I in ("%NODE_BIN%") do if %%~zI EQU 0 (',
      "  echo dotagents bundled node runtime invalid (empty): %NODE_BIN% 1>&2",
      "  exit /b 127",
      ")",
      `set "DOTAGENTS_CLI=%SCRIPT_DIR%${cliPath}"`,
      '"%NODE_BIN%" "%DOTAGENTS_CLI%" %*',
    );
    const launcherContent = launcherLines.join("\r\n");
    fs.writeFileSync(launcherPath, `${launcherContent}\r\n`, "utf8");
    return launcherPath;
  }

  const cliPath = toPosixPath(dotagentsCliRelativePath);
  const launcherLines = [
    "#!/usr/bin/env sh",
    "set -eu",
    'SCRIPT_DIR="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"',
    `NODE_BIN="$SCRIPT_DIR/${launcherCandidates[0]}"`,
  ];
  for (const fallbackPath of launcherCandidates.slice(1)) {
    launcherLines.push(
      'if [ ! -x "$NODE_BIN" ] || [ ! -s "$NODE_BIN" ]; then',
      `  NODE_BIN="$SCRIPT_DIR/${fallbackPath}"`,
      "fi",
    );
  }
  launcherLines.push(
    'if [ ! -x "$NODE_BIN" ] || [ ! -s "$NODE_BIN" ]; then',
    '  echo "dotagents bundled node runtime missing or invalid: $NODE_BIN" >&2',
    "  exit 127",
    "fi",
    `DOTAGENTS_CLI=\"$SCRIPT_DIR/${cliPath}\"`,
    'exec "$NODE_BIN" "$DOTAGENTS_CLI" "$@"',
  );
  const launcherContent = launcherLines.join("\n");
  fs.writeFileSync(launcherPath, `${launcherContent}\n`, "utf8");
  fs.chmodSync(launcherPath, 0o755);
  return launcherPath;
}

function copyRuntimeTree(cacheInstallDir, targetDir) {
  const sourceNodeModulesDir = path.join(cacheInstallDir, "node_modules");
  const destinationNodeModulesDir = path.join(targetDir, "node_modules");
  fs.cpSync(sourceNodeModulesDir, destinationNodeModulesDir, {
    recursive: true,
    dereference: true,
    force: true,
  });
}

function materializeRuntimeRoot(
  rootDir,
  target,
  cacheInstallDir,
  nodeBinaryRelativePath,
  dotagentsCliRelativePath,
) {
  ensureDirectory(rootDir);

  const targetDir = path.join(rootDir, target);
  removeDirectoryIfExists(targetDir);
  ensureDirectory(targetDir);

  copyRuntimeTree(cacheInstallDir, targetDir);

  const bundledNodeBinary = path.join(targetDir, nodeBinaryRelativePath);
  if (!isUsableBinaryFile(bundledNodeBinary)) {
    throw new Error(`Bundled node binary is missing at ${bundledNodeBinary}`);
  }
  const bundledDotagentsCli = path.join(targetDir, dotagentsCliRelativePath);
  if (!fs.existsSync(bundledDotagentsCli)) {
    throw new Error(
      `Bundled dotagents CLI is missing at ${bundledDotagentsCli}`,
    );
  }

  if (target !== "windows-x64") {
    fs.chmodSync(bundledNodeBinary, 0o755);
  }

  const launcherPath = writeLauncher(
    targetDir,
    target,
    nodeBinaryRelativePath,
    dotagentsCliRelativePath,
  );
  const launcherChecksum = sha256OfFile(launcherPath);

  const existingManifest = parseJsonFile(path.join(rootDir, "checksums.json"));
  const checksums =
    existingManifest &&
    typeof existingManifest === "object" &&
    existingManifest.checksums &&
    typeof existingManifest.checksums === "object" &&
    !Array.isArray(existingManifest.checksums)
      ? { ...existingManifest.checksums }
      : {};
  checksums[target] = launcherChecksum;

  writeJson(path.join(rootDir, "checksums.json"), {
    version: 1,
    dotagentsVersion: DOTAGENTS_VERSION,
    nodeVersion: NODE_VERSION,
    checksums,
  });

  return {
    targetDir,
    launcherPath,
  };
}

function main() {
  const hostTarget = detectHostTarget();
  const cacheInstallDir = path.join(
    CACHE_ROOT,
    `${hostTarget}-dotagents-${DOTAGENTS_VERSION}-node-${NODE_VERSION}`,
  );

  ensureDirectory(CACHE_ROOT);
  const { nodeBinaryRelativePath, dotagentsCliRelativePath } = ensureCacheReady(
    cacheInstallDir,
    hostTarget,
  );

  const tauriBundleResult = materializeRuntimeRoot(
    TAURI_BUNDLE_ROOT,
    hostTarget,
    cacheInstallDir,
    nodeBinaryRelativePath,
    dotagentsCliRelativePath,
  );
  const devRuntimeResult = materializeRuntimeRoot(
    DEV_RUNTIME_ROOT,
    hostTarget,
    cacheInstallDir,
    nodeBinaryRelativePath,
    dotagentsCliRelativePath,
  );

  console.log(
    `[dotagents] Prepared bundled runtime for ${hostTarget}:`,
    `\n  - ${tauriBundleResult.launcherPath}`,
    `\n  - ${devRuntimeResult.launcherPath}`,
  );
}

main();
