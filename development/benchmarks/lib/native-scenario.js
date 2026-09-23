"use strict";

const crypto = require("node:crypto");
const fs = require("node:fs");
const path = require("node:path");
const { runChild } = require("./process-scenario");

function digest(buffer) {
  return crypto.createHash("sha256").update(buffer).digest("hex");
}

async function buildNativeScenario(compiler, sourceRoot, output, repository) {
  const environment = { ...process.env };
  delete environment.NOCTER_HOME;
  const result = await runChild(
    compiler,
    ["build", "--root", sourceRoot, "--locked", "--offline", "--output", output],
    {
      cwd: repository,
      env: environment,
      stdio: ["ignore", "pipe", "pipe"],
    },
  );
  if (result.code !== 0 || result.signal !== null) {
    throw new Error(
      `${compiler} failed to build ${sourceRoot}: code ${result.code}, signal ${result.signal}\n` +
        result.stderr,
    );
  }
  const bytes = fs.readFileSync(output);
  return { bytes: bytes.length, sha256: digest(bytes) };
}

async function runNativeScenario(executable, invocation) {
  const result = await runChild(executable, invocation.args, {
    cwd: invocation.cwd,
    env: { ...process.env, ...invocation.environment },
    stdio: ["ignore", "pipe", "pipe"],
  });
  if (result.code !== 0 || result.signal !== null) {
    throw new Error(
      `${path.basename(executable)} ${invocation.args.join(" ")} exited with code ` +
        `${result.code} and signal ${result.signal}\n${result.stderr}`,
    );
  }
  invocation.validate(result);
  return { elapsed_ms: result.elapsedMs };
}

module.exports = { buildNativeScenario, digest, runNativeScenario };
