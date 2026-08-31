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

if (process.argv.length !== 7) {
  fail(
    "usage: render-manifest.js RELEASE VERSION COMPILER_SHA256 STD_TREE_SHA256 OUTPUT",
  );
}

const [releasePath, versionPath, compilerDigest, standardDigest, outputPath] =
  process.argv.slice(2);
const versionText = fs.readFileSync(versionPath, "utf8");
if (!/^\d+\.\d+\.\d+\n$/.test(versionText)) {
  fail("VERSION must contain one numeric semantic-version line");
}
const version = versionText.slice(0, -1);
const digestPattern = /^[0-9a-f]{64}$/;
if (!digestPattern.test(compilerDigest)) {
  fail("compiler digest must be lowercase SHA-256");
}
if (!digestPattern.test(standardDigest)) {
  fail("standard-library tree digest must be lowercase SHA-256");
}

let release;
try {
  release = JSON.parse(fs.readFileSync(releasePath, "utf8"));
} catch (error) {
  fail(`RELEASE.json is not valid JSON: ${error.message}`);
}

exactKeys(
  release,
  [
    "schema",
    "schema_version",
    "host",
    "default_target",
    "license",
    "implemented_targets",
    "archive",
  ],
  "release",
);
exactKeys(release.license, ["id", "path", "notice"], "license");
exactKeys(release.archive, ["root"], "archive");
if (!Array.isArray(release.implemented_targets) || release.implemented_targets.length !== 1) {
  fail("implemented_targets must contain exactly arm64-darwin");
}
const target = release.implemented_targets[0];
exactKeys(target, ["name", "backend", "executable", "os"], "implemented target");

const expectedArchive = `nocter-v${version}-arm64-darwin.tar.gz`;
const expected = [
  [release.schema, "nocter.release", "schema"],
  [release.schema_version, 1, "schema_version"],
  [release.host, "arm64-darwin", "host"],
  [release.default_target, "arm64-darwin", "default_target"],
  [release.license.id, "Apache-2.0", "license.id"],
  [release.license.path, "LICENSE", "license.path"],
  [release.license.notice, "NOTICE", "license.notice"],
  [target.name, "arm64-darwin", "implemented_targets[0].name"],
  [target.backend, "arm64", "implemented_targets[0].backend"],
  [target.executable, "macho", "implemented_targets[0].executable"],
  [target.os, "darwin", "implemented_targets[0].os"],
  [release.archive.root, ".nocter", "archive.root"],
];
for (const [actual, wanted, label] of expected) {
  if (actual !== wanted) {
    fail(`${label} must be ${JSON.stringify(wanted)}`);
  }
}

const manifest = {
  schema: "nocter.manifest",
  schema_version: 2,
  release: version,
  host: release.host,
  default_target: release.default_target,
  compiler: {
    path: "nocter",
    sha256: compilerDigest,
  },
  std: {
    path: "std",
    tree_sha256: standardDigest,
  },
  license: release.license,
  implemented_targets: release.implemented_targets,
  archive: {
    name: expectedArchive,
    root: release.archive.root,
  },
};
fs.writeFileSync(outputPath, `${JSON.stringify(manifest, null, 2)}\n`);
