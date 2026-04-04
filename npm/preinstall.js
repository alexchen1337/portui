"use strict";

const { existsSync, rmSync } = require("fs");
const { join } = require("path");

const bin = join(__dirname, "..", "bin", "port-cli");
if (existsSync(bin)) {
  rmSync(bin);
}
