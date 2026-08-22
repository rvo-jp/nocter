#!/usr/bin/env node

const fs = require("node:fs");

function fail(message) {
  process.stderr.write(`release manifest error: ${message}\n`);
  process.exit(1);
}

function exactKeys(value, expected, label) {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    fail(`${label} must be an object`);
  }
  const actual = Object.keys(value).sort();
  const wanted = [...expected].sort();
  if (JSON.stringify(actual) !== JSON.stringify(wanted)) {
    fail(`${label} keys must be exactly ${wanted.join(", ")}`);
  }
}

if (process.argv.length !== 4) {
  fail("usage: validate-manifest.js MANIFEST VERSION");
}

const manifestPath = process.argv[2];
const versionPath = process.argv[3];
const versionText = fs.readFileSync(versionPath, "utf8");
if (!/^\d+\.\d+\.\d+\n$/.test(versionText)) {
  fail("VERSION must contain one numeric semantic-version line");
}
const version = versionText.slice(0, -1);

let manifest;
try {
  manifest = JSON.parse(fs.readFileSync(manifestPath, "utf8"));
} catch (error) {
  fail(`MANIFEST.json is not valid JSON: ${error.message}`);
}

exactKeys(
  manifest,
  [
    "schema",
    "schema_version",
    "release",
    "host",
    "default_target",
    "compiler",
    "std",
    "license",
    "implemented_targets",
    "archive",
  ],
  "manifest",
);
exactKeys(manifest.compiler, ["path"], "compiler");
exactKeys(manifest.std, ["path"], "std");
exactKeys(manifest.license, ["id", "path", "notice"], "license");
exactKeys(manifest.archive, ["name", "root"], "archive");

if (!Array.isArray(manifest.implemented_targets) || manifest.implemented_targets.length !== 1) {
  fail("implemented_targets must contain exactly arm64-darwin");
}
const target = manifest.implemented_targets[0];
exactKeys(target, ["name", "backend", "executable", "os"], "implemented target");

const expectedArchive = `nocter-v${version}-arm64-darwin.tar.gz`;
const expected = [
  [manifest.schema, "nocter.manifest", "schema"],
  [manifest.schema_version, 1, "schema_version"],
  [manifest.release, version, "release"],
  [manifest.host, "arm64-darwin", "host"],
  [manifest.default_target, "arm64-darwin", "default_target"],
  [manifest.compiler.path, "nocter", "compiler.path"],
  [manifest.std.path, "std", "std.path"],
  [manifest.license.id, "Apache-2.0", "license.id"],
  [manifest.license.path, "LICENSE", "license.path"],
  [manifest.license.notice, "NOTICE", "license.notice"],
  [target.name, "arm64-darwin", "implemented_targets[0].name"],
  [target.backend, "arm64", "implemented_targets[0].backend"],
  [target.executable, "macho", "implemented_targets[0].executable"],
  [target.os, "darwin", "implemented_targets[0].os"],
  [manifest.archive.name, expectedArchive, "archive.name"],
  [manifest.archive.root, ".nocter", "archive.root"],
];
for (const [actual, wanted, label] of expected) {
  if (actual !== wanted) {
    fail(`${label} must be ${JSON.stringify(wanted)}`);
  }
}

