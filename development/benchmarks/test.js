#!/usr/bin/env node
"use strict";

const assert = require("node:assert/strict");
const path = require("node:path");
require("./run");
require("./lib/lsp-client");
require("./lib/lsp-scenario");
const { sourceIdentity } = require("./lib/source-identity");
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
assert.ok(
  sourceIdentity(path.resolve(__dirname, "../..")).package_check.some(
    (entry) => entry.path === "examples/text-search/index.nct",
  ),
);

process.stdout.write("Performance measurement helpers passed.\n");
