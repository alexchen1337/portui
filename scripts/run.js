#!/usr/bin/env node
"use strict";

const { spawnSync } = require("child_process");
const { existsSync } = require("fs");
const { join } = require("path");

const bin = join(__dirname, "..", "bin", "portui");

if (!existsSync(bin)) {
  console.error(
    "portui binary not found. On macOS, reinstall the package so postinstall can download it, " +
      "or build from source: cargo build --release && cp target/release/portui bin/portui"
  );
  process.exit(1);
}

const result = spawnSync(bin, process.argv.slice(2), {
  cwd: process.cwd(),
  stdio: "inherit",
});

process.exit(result.status ?? 1);
