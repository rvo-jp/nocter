#!/usr/bin/env node
"use strict";

const fs = require("node:fs");
const path = require("node:path");
const process = require("node:process");

const { readPinnedInputs } = require("./lib/manifest");
const { buildCorpus } = require("./lib/ucd");
const { renderProduct } = require("./lib/render");
const { validateProduct } = require("./lib/validate");

const root = __dirname;
const repository = path.resolve(root, "..", "..");
const output = path.join(repository, "development", "std", "internal", "unicode", "tables.nct");

function replaceAtomically(target, contents) {
  const current = fs.existsSync(target) ? fs.readFileSync(target, "utf8") : null;
  if (current === contents) return false;
  fs.mkdirSync(path.dirname(target), { recursive: true });
  const temporary = `${target}.tmp-${process.pid}`;
  try {
    fs.writeFileSync(temporary, contents, { encoding: "utf8", flag: "wx" });
    fs.renameSync(temporary, target);
  } finally {
    if (fs.existsSync(temporary)) fs.unlinkSync(temporary);
  }
  return true;
}

function main() {
  const mode = process.argv[2];
  if ((mode !== "--check" && mode !== "--write") || process.argv.length !== 3) {
    throw new Error("usage: node development/unicode/generate.js (--check|--write)");
  }
  const pinned = readPinnedInputs(root);
  const { product, oracle } = buildCorpus(pinned.contents);
  const summary = validateProduct(product, oracle);
  const rendered = renderProduct(pinned.version, product, summary);

  if (mode === "--check") {
    if (!fs.existsSync(output) || fs.readFileSync(output, "utf8") !== rendered) {
      throw new Error("generated Unicode tables differ; run with --write and review the result");
    }
    process.stdout.write(`Unicode ${pinned.version} tables are reproducible\n`);
    return;
  }
  const changed = replaceAtomically(output, rendered);
  process.stdout.write(changed ? `updated ${path.relative(repository, output)}\n` : "Unicode tables are unchanged\n");
}

try {
  main();
} catch (error) {
  process.stderr.write(`${error.message}\n`);
  process.exitCode = 1;
}
