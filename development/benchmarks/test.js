#!/usr/bin/env node
"use strict";

const assert = require("node:assert/strict");
require("./run");
require("./lib/lsp-client");
require("./lib/lsp-scenario");
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

process.stdout.write("Performance measurement helpers passed.\n");
