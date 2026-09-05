"use strict";

const crypto = require("node:crypto");
const fs = require("node:fs");
const path = require("node:path");

function sha256(file) {
  return crypto.createHash("sha256").update(fs.readFileSync(file)).digest("hex");
}

function regularFiles(directory) {
  return fs
    .readdirSync(directory, { withFileTypes: true })
    .flatMap((entry) => {
      const child = path.join(directory, entry.name);
      if (entry.isDirectory()) return regularFiles(child);
      return entry.isFile() ? [child] : [];
    })
    .sort();
}

function sourceIdentity(repository) {
  const singleFile = path.join(repository, "examples", "hello.nct");
  const packageRoot = path.join(repository, "examples", "text-search");
  const editorFile = path.join(repository, "examples", "text-banner", "banner.nct");
  return {
    single_file_check: {
      path: path.relative(repository, singleFile),
      sha256: sha256(singleFile),
    },
    package_check: regularFiles(packageRoot).map((file) => ({
      path: path.relative(repository, file),
      sha256: sha256(file),
    })),
    lsp_body_edit: {
      path: path.relative(repository, editorFile),
      sha256: sha256(editorFile),
    },
  };
}

module.exports = { sourceIdentity };
