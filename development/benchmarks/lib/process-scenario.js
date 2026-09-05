"use strict";

const { spawn } = require("node:child_process");

function parseDarwinResources(stderr) {
  const resident = /^\s*(\d+)\s+maximum resident set size$/m.exec(stderr);
  return resident ? { maximum_resident_bytes: Number(resident[1]) } : null;
}

function runChild(command, args, options) {
  return new Promise((resolve, reject) => {
    const started = process.hrtime.bigint();
    const child = spawn(command, args, options);
    const stdout = [];
    const stderr = [];
    child.stdout.on("data", (chunk) => stdout.push(chunk));
    child.stderr.on("data", (chunk) => stderr.push(chunk));
    child.on("error", reject);
    child.on("close", (code, signal) => {
      const elapsedMs = Number(process.hrtime.bigint() - started) / 1_000_000;
      resolve({
        code,
        signal,
        elapsedMs,
        stdout: Buffer.concat(stdout).toString("utf8"),
        stderr: Buffer.concat(stderr).toString("utf8"),
      });
    });
  });
}

async function runProcessScenario(compiler, args, cwd) {
  const environment = { ...process.env };
  delete environment.NOCTER_HOME;

  const timed = process.platform === "darwin";
  const command = timed ? "/usr/bin/time" : compiler;
  const commandArgs = timed ? ["-l", compiler, ...args] : args;
  const result = await runChild(command, commandArgs, {
    cwd,
    env: environment,
    stdio: ["ignore", "pipe", "pipe"],
  });
  if (result.code !== 0 || result.signal !== null) {
    throw new Error(
      `${compiler} ${args.join(" ")} exited with code ${result.code} and signal ${result.signal}\n` +
        result.stderr,
    );
  }
  return {
    elapsed_ms: result.elapsedMs,
    resources: timed ? parseDarwinResources(result.stderr) : null,
  };
}

module.exports = { parseDarwinResources, runProcessScenario };
