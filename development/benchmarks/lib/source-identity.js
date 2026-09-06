"use strict";

const crypto = require("node:crypto");
const fs = require("node:fs");
const path = require("node:path");

function sha256(file) {
  return crypto.createHash("sha256").update(fs.readFileSync(file)).digest("hex");
}

function sourceFiles(directory) {
  return fs
    .readdirSync(directory, { withFileTypes: true })
    .flatMap((entry) => {
      const child = path.join(directory, entry.name);
      if (entry.isDirectory()) return sourceFiles(child);
      return entry.isFile() && path.extname(child) === ".nct" ? [child] : [];
    })
    .sort();
}

function identities(repository, directory) {
  return sourceFiles(directory).map((file) => ({
    path: path.relative(repository, file),
    sha256: sha256(file),
  }));
}

function sourceIdentity(repository) {
  const singleFile = path.join(repository, "examples", "hello.nct");
  const packageRoot = path.join(repository, "examples", "text-search");
  const editorRoot = path.join(repository, "examples", "text-banner");
  return {
    single_file_check: {
      path: path.relative(repository, singleFile),
      sha256: sha256(singleFile),
    },
    package_check: identities(repository, packageRoot),
    lsp_body_edit: identities(repository, editorRoot),
  };
}

module.exports = { sourceIdentity };
