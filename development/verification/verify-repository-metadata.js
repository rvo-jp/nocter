#!/usr/bin/env node

const fs = require("node:fs");
const path = require("node:path");
const { spawnSync } = require("node:child_process");

const repositoryRoot = path.resolve(__dirname, "../..");
const compilerManifest = path.join(repositoryRoot, "development/compiler/Cargo.toml");
const releasePath = path.join(repositoryRoot, "development/packaging/RELEASE.json");

function fail(message) {
  process.stderr.write(`repository metadata error: ${message}\n`);
  process.exit(1);
}

let release;
try {
  release = JSON.parse(fs.readFileSync(releasePath, "utf8"));
} catch (error) {
  fail(`cannot read RELEASE.json: ${error.message}`);
}

const license = release?.license;
if (
  license === null ||
  typeof license !== "object" ||
  Array.isArray(license) ||
  typeof license.id !== "string" ||
  typeof license.path !== "string" ||
  typeof license.notice !== "string"
) {
  fail("RELEASE.json must define string license.id, license.path, and license.notice fields");
}

const licensePath = path.join(repositoryRoot, license.path);
const noticePath = path.join(repositoryRoot, license.notice);
let licenseText;
try {
  licenseText = fs.readFileSync(licensePath, "utf8");
} catch (error) {
  fail(`cannot read declared license file ${license.path}: ${error.message}`);
}
try {
  if (fs.readFileSync(noticePath, "utf8").trim().length === 0) {
    fail(`declared notice file ${license.notice} is empty`);
  }
} catch (error) {
  fail(`cannot read declared notice file ${license.notice}: ${error.message}`);
}

const knownLicenseMarkers = new Map([
  ["Apache-2.0", ["Apache License", "Version 2.0, January 2004"]],
]);
const requiredMarkers = knownLicenseMarkers.get(license.id);
if (!requiredMarkers) {
  fail(`license.id ${JSON.stringify(license.id)} has no repository verifier`);
}
for (const marker of requiredMarkers) {
  if (!licenseText.includes(marker)) {
    fail(`${license.path} does not contain the marker required by ${license.id}: ${marker}`);
  }
}

const metadataResult = spawnSync(
  "cargo",
  [
    "metadata",
    "--locked",
    "--no-deps",
    "--format-version",
    "1",
    "--manifest-path",
    compilerManifest,
  ],
  { encoding: "utf8" },
);
if (metadataResult.error) {
  fail(`cannot execute cargo metadata: ${metadataResult.error.message}`);
}
if (metadataResult.status !== 0) {
  process.stderr.write(metadataResult.stderr);
  fail(`cargo metadata exited with status ${metadataResult.status}`);
}

let metadata;
try {
  metadata = JSON.parse(metadataResult.stdout);
} catch (error) {
  fail(`cargo metadata returned invalid JSON: ${error.message}`);
}

const workspaceMembers = new Set(metadata.workspace_members);
const workspacePackages = metadata.packages.filter((entry) => workspaceMembers.has(entry.id));
if (workspacePackages.length !== workspaceMembers.size) {
  fail("cargo metadata omitted one or more workspace members");
}
const mismatches = workspacePackages
  .filter((entry) => entry.license !== license.id)
  .map((entry) => `${entry.name} (${entry.license ?? "unset"})`)
  .sort();
if (mismatches.length > 0) {
  fail(
    `Cargo package licenses must match RELEASE.json ${license.id}: ${mismatches.join(", ")}`,
  );
}

process.stdout.write(
  `Repository license metadata verified for ${workspacePackages.length} Cargo packages (${license.id}).\n`,
);
