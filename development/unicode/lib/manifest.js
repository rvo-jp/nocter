"use strict";

const crypto = require("node:crypto");
const fs = require("node:fs");
const path = require("node:path");

const EXPECTED_ROLES = [
  "unicodeData",
  "derivedCoreProperties",
  "propertyList",
  "specialCasing",
  "license",
];

function sha256(bytes) {
  return crypto.createHash("sha256").update(bytes).digest("hex");
}

function requireExactKeys(value, expected, context) {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    throw new Error(`${context} must be an object`);
  }
  const actual = Object.keys(value).sort();
  const wanted = [...expected].sort();
  if (actual.join("\0") !== wanted.join("\0")) {
    throw new Error(`${context} fields must be exactly: ${wanted.join(", ")}`);
  }
}

function readManifest(root) {
  const manifestPath = path.join(root, "manifest.json");
  const manifest = JSON.parse(fs.readFileSync(manifestPath, "utf8"));
  requireExactKeys(manifest, ["schema", "unicodeVersion", "inputs"], "manifest");
  if (manifest.schema !== 1) throw new Error("unsupported Unicode manifest schema");
  if (manifest.unicodeVersion !== "17.0.0") {
    throw new Error(`unsupported Unicode version ${JSON.stringify(manifest.unicodeVersion)}`);
  }
  if (!Array.isArray(manifest.inputs) || manifest.inputs.length !== EXPECTED_ROLES.length) {
    throw new Error("manifest must contain exactly the required Unicode inputs");
  }

  const inputs = new Map();
  for (const [index, entry] of manifest.inputs.entries()) {
    requireExactKeys(entry, ["role", "file", "source", "bytes", "sha256"], `inputs[${index}]`);
    if (!EXPECTED_ROLES.includes(entry.role) || inputs.has(entry.role)) {
      throw new Error(`invalid or duplicate Unicode input role ${JSON.stringify(entry.role)}`);
    }
    if (path.basename(entry.file) !== entry.file || entry.file.length === 0) {
      throw new Error(`Unicode input file must be a plain file name: ${JSON.stringify(entry.file)}`);
    }
    if (typeof entry.source !== "string" || !entry.source.startsWith("https://www.unicode.org/")) {
      throw new Error(`Unicode input source is not canonical: ${JSON.stringify(entry.source)}`);
    }
    if (!Number.isSafeInteger(entry.bytes) || entry.bytes <= 0) {
      throw new Error(`Unicode input byte length is invalid for ${entry.file}`);
    }
    if (typeof entry.sha256 !== "string" || !/^[0-9a-f]{64}$/.test(entry.sha256)) {
      throw new Error(`Unicode input digest is invalid for ${entry.file}`);
    }
    inputs.set(entry.role, entry);
  }
  for (const role of EXPECTED_ROLES) {
    if (!inputs.has(role)) throw new Error(`Unicode input role ${role} is absent`);
  }
  return { manifest, inputs };
}

function readPinnedInputs(root) {
  const { manifest, inputs } = readManifest(root);
  const directory = path.join(root, "inputs", manifest.unicodeVersion);
  const contents = new Map();
  for (const [role, entry] of inputs) {
    const inputPath = path.join(directory, entry.file);
    const bytes = fs.readFileSync(inputPath);
    if (bytes.length !== entry.bytes) {
      throw new Error(`${entry.file} has ${bytes.length} bytes; expected ${entry.bytes}`);
    }
    const digest = sha256(bytes);
    if (digest !== entry.sha256) {
      throw new Error(`${entry.file} has SHA-256 ${digest}; expected ${entry.sha256}`);
    }
    const text = bytes.toString("utf8");
    if (!Buffer.from(text, "utf8").equals(bytes)) {
      throw new Error(`${entry.file} is not canonical UTF-8`);
    }
    contents.set(role, text);
  }
  return { version: manifest.unicodeVersion, contents };
}

module.exports = { readPinnedInputs, sha256 };
