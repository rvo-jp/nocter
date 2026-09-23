#!/usr/bin/env node
"use strict";

const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { compilerIdentity, hostIdentity, repositoryIdentity } = require("./lib/benchmark-identity");
const { buildNativeScenario, runNativeScenario } = require("./lib/native-scenario");
const { nativeWorkloads } = require("./lib/native-workloads");
const { summarize } = require("./lib/statistics");

function fail(message) {
  process.stderr.write(`native benchmark failed: ${message}\n`);
  process.exit(1);
}

function positiveInteger(text, option) {
  const value = Number(text);
  if (!Number.isSafeInteger(value) || value < 1) fail(`${option} requires a positive integer`);
  return value;
}

function parseArguments(argv, repository) {
  const options = { compilers: [], samples: 7, warmups: 2, output: null };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    const value = argv[index + 1];
    if (argument === "--compiler") {
      if (!value) fail("--compiler requires LABEL=PATH");
      const separator = value.indexOf("=");
      if (separator <= 0 || separator === value.length - 1) fail("--compiler requires LABEL=PATH");
      options.compilers.push({
        label: value.slice(0, separator),
        binary: path.resolve(repository, value.slice(separator + 1)),
      });
      index += 1;
    } else if (argument === "--samples") {
      options.samples = positiveInteger(value, argument);
      index += 1;
    } else if (argument === "--warmups") {
      options.warmups = positiveInteger(value, argument);
      index += 1;
    } else if (argument === "--output") {
      if (!value) fail("--output requires a path");
      options.output = path.resolve(repository, value);
      index += 1;
    } else {
      fail(`unknown option ${argument}`);
    }
  }
  if (options.compilers.length === 0) {
    options.compilers.push({ label: "installed", binary: path.join(repository, "dist/.nocter/nocter") });
  }
  const labels = new Set();
  for (const compiler of options.compilers) {
    if (!/^[A-Za-z0-9][A-Za-z0-9._-]*$/.test(compiler.label)) {
      fail(`compiler label is not path-safe: ${compiler.label}`);
    }
    if (labels.has(compiler.label)) fail(`duplicate compiler label ${compiler.label}`);
    labels.add(compiler.label);
    if (!fs.statSync(compiler.binary).isFile()) fail(`compiler is not a file: ${compiler.binary}`);
  }
  return options;
}

async function prepare(options, repository, temporary, workloads) {
  const prepared = new Map();
  const artifacts = {};
  for (const workload of workloads) artifacts[workload.name] = {};
  for (const compiler of options.compilers) {
    for (const workload of workloads) {
      const root = path.join(temporary, compiler.label, workload.name);
      fs.mkdirSync(root, { recursive: true });
      workload.prepare(root);
      const executable = path.join(root, workload.name);
      artifacts[workload.name][compiler.label] = await buildNativeScenario(
        compiler.binary,
        workload.sourceRoot,
        executable,
        repository,
      );
      prepared.set(`${compiler.label}:${workload.name}`, { executable, root });
    }
  }
  return { artifacts, prepared };
}

async function measure(options, repository, workloads, prepared) {
  const output = {};
  for (const workload of workloads) {
    output[workload.name] = {};
    for (const compiler of options.compilers) {
      const candidate = prepared.get(`${compiler.label}:${workload.name}`);
      for (let index = 0; index < options.warmups; index += 1) {
        workload.reset(candidate.root);
        await runNativeScenario(
          candidate.executable,
          workload.invocation(candidate.root, repository),
        );
      }
    }
    const samples = new Map(options.compilers.map((compiler) => [compiler.label, []]));
    for (let round = 0; round < options.samples; round += 1) {
      const order = [...options.compilers];
      if (round % 2 === 1) order.reverse();
      for (const [position, compiler] of order.entries()) {
        const candidate = prepared.get(`${compiler.label}:${workload.name}`);
        workload.reset(candidate.root);
        const sample = await runNativeScenario(
          candidate.executable,
          workload.invocation(candidate.root, repository),
        );
        samples.get(compiler.label).push({
          round,
          position,
          elapsed_ms: sample.elapsed_ms,
        });
      }
    }
    for (const compiler of options.compilers) {
      output[workload.name][compiler.label] = {
        elapsed_ms: summarize(samples.get(compiler.label).map((sample) => sample.elapsed_ms)),
        samples: samples.get(compiler.label),
      };
    }
  }
  return output;
}

async function main() {
  const repository = path.resolve(__dirname, "../..");
  const options = parseArguments(process.argv.slice(2), repository);
  const workloads = nativeWorkloads(repository);
  const temporary = fs.mkdtempSync(path.join(os.tmpdir(), "nocter-native-benchmark-"));
  try {
    const compilers = options.compilers.map(compilerIdentity);
    const { artifacts, prepared } = await prepare(options, repository, temporary, workloads);
    const repositoryState = repositoryIdentity(repository);
    const result = {
      schema: "nocter.native_performance",
      schema_version: 1,
      recorded_at: new Date().toISOString(),
      repository_revision: repositoryState.revision,
      repository_clean: repositoryState.clean,
      host: hostIdentity(),
      configuration: { warmups: options.warmups, samples: options.samples },
      compilers,
      workloads: workloads.map((workload) => ({
        name: workload.name,
        categories: workload.categories,
        sources: workload.sources,
        inputs: workload.inputs,
      })),
      artifacts,
      runtime: await measure(options, repository, workloads, prepared),
    };
    const serialized = `${JSON.stringify(result, null, 2)}\n`;
    if (options.output) fs.writeFileSync(options.output, serialized);
    else process.stdout.write(serialized);
  } finally {
    fs.rmSync(temporary, { recursive: true, force: true });
  }
}

if (require.main === module) main().catch((error) => fail(error.stack ?? error.message));

module.exports = { parseArguments };
