#!/usr/bin/env node
"use strict";

const assert = require("node:assert/strict");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");

const { readPinnedInputs, sha256 } = require("./lib/manifest");

const roles = [
  ["unicodeData", "UnicodeData.txt"],
  ["derivedCoreProperties", "DerivedCoreProperties.txt"],
  ["propertyList", "PropList.txt"],
  ["specialCasing", "SpecialCasing.txt"],
  ["license", "LICENSE.txt"],
];

const root = fs.mkdtempSync(path.join(os.tmpdir(), "nocter-unicode-generator-"));
try {
  const inputRoot = path.join(root, "inputs", "17.0.0");
  fs.mkdirSync(inputRoot, { recursive: true });
  const inputs = roles.map(([role, file], index) => {
    const bytes = Buffer.from(`pinned-${index}\n`, "utf8");
    fs.writeFileSync(path.join(inputRoot, file), bytes);
    return {
      role,
      file,
      source: `https://www.unicode.org/Public/17.0.0/ucd/${file}`,
      bytes: bytes.length,
      sha256: sha256(bytes),
    };
  });
  fs.writeFileSync(
    path.join(root, "manifest.json"),
    `${JSON.stringify({ schema: 1, unicodeVersion: "17.0.0", inputs }, null, 2)}\n`,
  );

  assert.equal(readPinnedInputs(root).contents.size, roles.length);
  fs.writeFileSync(path.join(inputRoot, "UnicodeData.txt"), "tamper-0\n");
  assert.throws(
    () => readPinnedInputs(root),
    /UnicodeData\.txt has SHA-256/,
    "same-length input alteration must invalidate the manifest",
  );
  process.stdout.write("Unicode manifest mutation test passed\n");
} finally {
  fs.rmSync(root, { recursive: true, force: true });
}
