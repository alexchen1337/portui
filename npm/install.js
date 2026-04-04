"use strict";

const { existsSync, mkdirSync, copyFileSync, chmodSync } = require("fs");
const { join } = require("path");
const { pipeline } = require("stream/promises");
const { createGunzip } = require("zlib");
const { Readable } = require("stream");
const tar = require("tar");

const root = join(__dirname, "..");
const binDir = join(root, "bin");
const binPath = join(binDir, "port-cli");

async function downloadRelease(url) {
  const res = await fetch(url);
  if (!res.ok) {
    throw new Error(`Failed to download ${url} (${res.status} ${res.statusText})`);
  }
  mkdirSync(binDir, { recursive: true });
  await pipeline(
    Readable.fromWeb(res.body),
    createGunzip(),
    tar.x({ strip: 1, C: binDir })
  );
  chmodSync(binPath, 0o755);
}

async function main() {
  if (process.env.PORT_CLI_SKIP_DOWNLOAD === "1") {
    return;
  }

  const devBuild = join(root, "target", "release", "port-cli");
  if (existsSync(devBuild)) {
    mkdirSync(binDir, { recursive: true });
    copyFileSync(devBuild, binPath);
    chmodSync(binPath, 0o755);
    console.log("port-cli: using local build from target/release");
    return;
  }

  if (process.platform !== "darwin") {
    console.warn(
      "port-cli: macOS only. Skipping prebuilt download. Build from source with Rust: cargo install --path ."
    );
    return;
  }

  const arch =
    process.arch === "arm64" ? "darwin-arm64" : "darwin-x64";
  const { version } = require("../package.json");
  const url = `https://github.com/alexchen1337/port-cli/releases/download/v${version}/port-cli-${arch}.tar.gz`;

  console.error(`Downloading port-cli ${version} for ${arch}…`);
  await downloadRelease(url);
  console.error("port-cli: installed");
}

main().catch((err) => {
  console.error(err.message || err);
  process.exit(1);
});
