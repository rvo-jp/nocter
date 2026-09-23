"use strict";

const crypto = require("node:crypto");
const fs = require("node:fs");
const os = require("node:os");
const { spawnSync } = require("node:child_process");
const { installedHomeIdentity } = require("./installed-home-identity");

function compilerIdentity(compiler) {
  const version = spawnSync(compiler.binary, ["--version"], { encoding: "utf8" });
  if (version.status !== 0) {
    throw new Error(`${compiler.binary} --version failed: ${version.stderr}`);
  }
  const bytes = fs.readFileSync(compiler.binary);
  const sha256 = crypto.createHash("sha256").update(bytes).digest("hex");
  return {
    label: compiler.label,
    sha256,
    version: version.stdout.trim().split("\n"),
    installed_home: installedHomeIdentity(compiler.binary, sha256),
  };
}

function repositoryIdentity(repository) {
  const revision = spawnSync("git", ["rev-parse", "HEAD"], { cwd: repository, encoding: "utf8" });
  const status = spawnSync("git", ["status", "--short"], { cwd: repository, encoding: "utf8" });
  return {
    revision: revision.status === 0 ? revision.stdout.trim() : null,
    clean: status.status === 0 ? status.stdout === "" : null,
  };
}

function hostIdentity() {
  return {
    platform: process.platform,
    release: os.release(),
    architecture: os.arch(),
    cpu: os.cpus()[0]?.model ?? null,
    logical_cpus: os.cpus().length,
    memory_bytes: os.totalmem(),
    node: process.version,
  };
}

module.exports = { compilerIdentity, hostIdentity, repositoryIdentity };
