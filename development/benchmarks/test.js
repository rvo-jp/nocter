#!/usr/bin/env node
"use strict";

const assert = require("node:assert/strict");
const crypto = require("node:crypto");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
require("./run");
require("./lib/lsp-client");
require("./lib/lsp-scenario");
const { sourceIdentity } = require("./lib/source-identity");
const { installedHomeIdentity } = require("./lib/installed-home-identity");
const { parseDarwinResources } = require("./lib/process-scenario");
const { summarize } = require("./lib/statistics");

assert.equal(summarize([]), null);
assert.deepEqual(summarize([9, 1, 5, 3, 7]), {
  count: 5,
  min: 1,
  median: 5,
  p90: 8.2,
  max: 9,
});
assert.deepEqual(
  parseDarwinResources("  196624384  maximum resident set size\n"),
  { maximum_resident_bytes: 196624384 },
);
assert.equal(parseDarwinResources("resource statistics unavailable\n"), null);
const sources = sourceIdentity(path.resolve(__dirname, "../.."));
assert.deepEqual(
  sources.package_check.map((entry) => entry.path),
  ["examples/text-search/index.nct", "examples/text-search/search.nct"],
);
assert.deepEqual(
  sources.lsp_body_edit.map((entry) => entry.path),
  ["examples/text-banner/banner.nct", "examples/text-banner/index.nct"],
);

const temporary = fs.mkdtempSync(path.join(os.tmpdir(), "nocter-benchmark-test-"));
try {
  const binary = path.join(temporary, "nocter");
  const compiler = Buffer.from("compiler");
  const compilerSha256 = crypto.createHash("sha256").update(compiler).digest("hex");
  fs.writeFileSync(binary, compiler);
  const manifest = Buffer.from(
    `${JSON.stringify({
      schema: "nocter.manifest",
      schema_version: 2,
      compiler: { path: "nocter", sha256: compilerSha256 },
      std: { path: "std", tree_sha256: "standard-tree" },
    })}\n`,
  );
  fs.writeFileSync(path.join(temporary, "MANIFEST.json"), manifest);
  assert.deepEqual(installedHomeIdentity(binary, compilerSha256), {
    manifest_sha256: crypto.createHash("sha256").update(manifest).digest("hex"),
    standard_library_tree_sha256: "standard-tree",
  });
  assert.throws(
    () => installedHomeIdentity(binary, "wrong-compiler"),
    /compiler digest does not match/,
  );
} finally {
  fs.rmSync(temporary, { recursive: true });
}

process.stdout.write("Performance measurement helpers passed.\n");
