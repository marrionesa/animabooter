#!/usr/bin/env node

import { existsSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const packageDir = join(dirname(fileURLToPath(import.meta.url)), "..");
const binaryName = process.platform === "win32" ? "animabooter.exe" : "animabooter";
const bundledBinary = join(packageDir, ".bin", binaryName);
const configuredBinary = process.env.ANIMABOOTER_BIN;
const executable = configuredBinary || bundledBinary;

if (!existsSync(executable)) {
  console.error(
    "AnimaBooter native binary is not installed. Reinstall the package or set ANIMABOOTER_BIN."
  );
  process.exit(1);
}

const result = spawnSync(executable, process.argv.slice(2), {
  stdio: "inherit"
});

if (result.error) {
  console.error(`Unable to start AnimaBooter: ${result.error.message}`);
  process.exit(1);
}

process.exit(result.status ?? 1);