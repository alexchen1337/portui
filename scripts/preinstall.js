"use strict";

const { existsSync, rmSync } = require("fs");
const { join } = require("path");

const bin = join(__dirname, "..", "bin", "portui");
if (existsSync(bin)) {
  rmSync(bin);
}
