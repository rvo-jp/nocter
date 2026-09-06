"use strict";

const crypto = require("node:crypto");
const fs = require("node:fs");
const path = require("node:path");

function sha256(bytes) {
  return crypto.createHash("sha256").update(bytes).digest("hex");
}

/**
 * Identifies the complete installed compiler home selected by one benchmark binary.
 *
 * Public commands validate the manifest and its referenced content themselves. The benchmark
 * records the same manifest authority so a result cannot silently compare different standard
 * libraries under compiler binaries that happen to report the same release name.
 */
function installedHomeIdentity(binary, compilerSha256) {
  const home = path.dirname(binary);
  const manifestPath = path.join(home, "MANIFEST.json");
  const bytes = fs.readFileSync(manifestPath);
  const manifest = JSON.parse(bytes.toString("utf8"));
  if (manifest.schema !== "nocter.manifest" || manifest.schema_version !== 2) {
    throw new Error(`unsupported installed manifest: ${manifestPath}`);
  }
  if (
    typeof manifest.compiler?.path !== "string" ||
    path.resolve(home, manifest.compiler.path) !== path.resolve(binary)
  ) {
    throw new Error(`installed manifest does not select benchmark compiler: ${binary}`);
  }
  if (manifest.compiler.sha256 !== compilerSha256) {
    throw new Error(`installed manifest compiler digest does not match: ${binary}`);
  }
  if (typeof manifest.std?.tree_sha256 !== "string") {
    throw new Error(`installed manifest has no standard-library identity: ${manifestPath}`);
  }
  return {
    manifest_sha256: sha256(bytes),
    standard_library_tree_sha256: manifest.std.tree_sha256,
  };
}

module.exports = { installedHomeIdentity };
