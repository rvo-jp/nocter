#!/usr/bin/env node
"use strict";

const assert = require("node:assert/strict");
const crypto = require("node:crypto");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
require("./run");
const { parseArguments: parseNativeArguments } = require("./native-run");
require("../verification/lsp-client");
require("./lib/lsp-scenario");
const { sourceIdentity } = require("./lib/source-identity");
const { installedHomeIdentity } = require("./lib/installed-home-identity");
const { parseDarwinResources } = require("./lib/process-scenario");
const { summarize } = require("./lib/statistics");
const { nativeWorkloads } = require("./lib/native-workloads");

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
assert.deepEqual(
  parseNativeArguments(
    ["--compiler", `candidate=${process.execPath}`, "--samples", "3", "--warmups", "1"],
    path.resolve(__dirname, "../.."),
  ),
  {
    compilers: [{ label: "candidate", binary: process.execPath }],
    samples: 3,
    warmups: 1,
    output: null,
  },
);
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
  const workloadRoot = path.join(temporary, "workloads");
  fs.mkdirSync(workloadRoot);
  const workloads = nativeWorkloads(path.resolve(__dirname, "../.."));
  assert.deepEqual(
    workloads.map((workload) => workload.name),
    [
      "line_frequency",
      "json_normalize",
      "archive_inspect",
      "binary_record",
      "subprocess_pipeline",
      "async_http",
      "http_service",
    ],
  );
  const categories = new Set(workloads.flatMap((workload) => workload.categories));
  for (const expected of [
    "synchronous",
    "fallible",
    "allocation",
    "asynchronous",
    "cancellation",
    "collections",
    "parsing",
    "compression",
    "filesystem",
    "process",
    "networking",
    "http",
  ]) {
    assert(categories.has(expected), `native workload coverage omitted ${expected}`);
  }
  for (const workload of workloads) {
    const root = path.join(workloadRoot, workload.name);
    fs.mkdirSync(root);
    workload.prepare(root);
    workload.reset(root);
    assert(workload.sources.length > 0, `${workload.name} has no source identity`);
    for (const input of workload.inputs) {
      assert.match(input.sha256, /^[0-9a-f]{64}$/);
      assert(input.bytes > 0);
    }
    const invocation = workload.invocation(root, workload.sourceRoot);
    assert(path.isAbsolute(invocation.cwd));
    assert(Array.isArray(invocation.args));
  }
  const subprocess = workloads.find((workload) => workload.name === "subprocess_pipeline");
  const detachedInvocation = subprocess.invocation;
  assert.equal(
    detachedInvocation(workloadRoot, subprocess.sourceRoot).cwd,
    subprocess.sourceRoot,
  );

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
