#!/usr/bin/env node
"use strict";

const crypto = require("node:crypto");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { spawnSync } = require("node:child_process");
const { runProcessScenario } = require("./lib/process-scenario");
const { runLspSession } = require("./lib/lsp-scenario");
const { sourceIdentity } = require("./lib/source-identity");
const { summarize } = require("./lib/statistics");

function fail(message) {
  process.stderr.write(`benchmark failed: ${message}\n`);
  process.exit(1);
}

function positiveInteger(text, option) {
  const value = Number(text);
  if (!Number.isSafeInteger(value) || value < 1) fail(`${option} requires a positive integer`);
  return value;
}

function parseArguments(argv, repository) {
  const options = {
    compilers: [],
    samples: 7,
    warmups: 2,
    lspSamples: 5,
    lspEdits: 50,
    output: null,
  };
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
    } else if (argument === "--lsp-samples") {
      options.lspSamples = positiveInteger(value, argument);
      index += 1;
    } else if (argument === "--lsp-edits") {
      options.lspEdits = positiveInteger(value, argument);
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
    if (labels.has(compiler.label)) fail(`duplicate compiler label ${compiler.label}`);
    labels.add(compiler.label);
    if (!fs.statSync(compiler.binary).isFile()) fail(`compiler is not a file: ${compiler.binary}`);
  }
  return options;
}

function compilerIdentity(compiler) {
  const version = spawnSync(compiler.binary, ["--version"], { encoding: "utf8" });
  if (version.status !== 0) fail(`${compiler.binary} --version failed: ${version.stderr}`);
  return {
    label: compiler.label,
    sha256: crypto.createHash("sha256").update(fs.readFileSync(compiler.binary)).digest("hex"),
    version: version.stdout.trim().split("\n"),
  };
}

function repositoryRevision(repository) {
  const result = spawnSync("git", ["rev-parse", "HEAD"], { cwd: repository, encoding: "utf8" });
  return result.status === 0 ? result.stdout.trim() : null;
}

function repositoryClean(repository) {
  const result = spawnSync("git", ["status", "--short"], { cwd: repository, encoding: "utf8" });
  return result.status === 0 ? result.stdout === "" : null;
}

function resourceSummary(samples) {
  const resident = samples
    .map((sample) => sample.resources?.maximum_resident_bytes)
    .filter((value) => value !== undefined);
  return resident.length === 0 ? null : { maximum_resident_bytes: summarize(resident) };
}

async function measureProcesses(options, repository) {
  const scenarios = [
    {
      name: "single_file_check",
      args: ["check", "--file", "examples/hello.nct", "--offline"],
    },
    {
      name: "package_check",
      args: ["check", "--root", "examples/text-search", "--locked", "--offline"],
    },
  ];
  const output = {};
  for (const scenario of scenarios) {
    output[scenario.name] = {};
    for (const compiler of options.compilers) {
      for (let index = 0; index < options.warmups; index += 1) {
        await runProcessScenario(compiler.binary, scenario.args, repository);
      }
    }
    const samples = new Map(options.compilers.map((compiler) => [compiler.label, []]));
    for (let round = 0; round < options.samples; round += 1) {
      const order = [...options.compilers];
      if (round % 2 === 1) order.reverse();
      for (const compiler of order) {
        samples
          .get(compiler.label)
          .push(await runProcessScenario(compiler.binary, scenario.args, repository));
      }
    }
    for (const compiler of options.compilers) {
      const compilerSamples = samples.get(compiler.label);
      output[scenario.name][compiler.label] = {
        elapsed_ms: summarize(compilerSamples.map((sample) => sample.elapsed_ms)),
        resources: resourceSummary(compilerSamples),
      };
    }
  }
  return output;
}

async function measureLsp(options, repository) {
  const output = {};
  for (const compiler of options.compilers) output[compiler.label] = [];
  for (let round = 0; round < options.lspSamples; round += 1) {
    const order = [...options.compilers];
    if (round % 2 === 1) order.reverse();
    for (const compiler of order) {
      output[compiler.label].push(
        await runLspSession(compiler.binary, repository, options.warmups, options.lspEdits),
      );
    }
  }
  for (const compiler of options.compilers) {
    const sessions = output[compiler.label];
    output[compiler.label] = {
      initialize_ms: summarize(sessions.map((session) => session.initialize_ms)),
      open_and_hover_ms: summarize(sessions.map((session) => session.open_and_hover_ms)),
      edit_and_hover_ms: summarize(sessions.flatMap((session) => session.edit_and_hover_ms)),
      session_total_ms: summarize(
        sessions.map((session) =>
          session.edit_and_hover_ms.reduce((total, elapsed) => total + elapsed, 0),
        ),
      ),
    };
  }
  return output;
}

async function main() {
  const repository = path.resolve(__dirname, "../..");
  const options = parseArguments(process.argv.slice(2), repository);
  const result = {
    schema: 1,
    recorded_at: new Date().toISOString(),
    repository_revision: repositoryRevision(repository),
    repository_clean: repositoryClean(repository),
    host: {
      platform: process.platform,
      release: os.release(),
      architecture: os.arch(),
      cpu: os.cpus()[0]?.model ?? null,
      logical_cpus: os.cpus().length,
      memory_bytes: os.totalmem(),
      node: process.version,
    },
    configuration: {
      process_warmups: options.warmups,
      process_samples: options.samples,
      lsp_sessions: options.lspSamples,
      lsp_warmup_edits_per_session: options.warmups,
      lsp_measured_edits_per_session: options.lspEdits,
    },
    compilers: options.compilers.map(compilerIdentity),
    source_inputs: sourceIdentity(repository),
    scenarios: {
      process: await measureProcesses(options, repository),
      lsp_body_edit: await measureLsp(options, repository),
    },
  };
  const serialized = `${JSON.stringify(result, null, 2)}\n`;
  if (options.output) fs.writeFileSync(options.output, serialized);
  else process.stdout.write(serialized);
}

if (require.main === module) {
  main().catch((error) => fail(error.stack ?? error.message));
}

module.exports = { parseArguments };
