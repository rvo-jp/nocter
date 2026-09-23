"use strict";

const fs = require("node:fs");
const path = require("node:path");
const zlib = require("node:zlib");
const { digest } = require("./native-scenario");
const { identities } = require("./source-identity");

function remove(root, names) {
  for (const name of names) fs.rmSync(path.join(root, name), { force: true, recursive: true });
}

function textInvocation(root, args, validate) {
  return { args, cwd: root, environment: {}, validate };
}

function quiet(result) {
  if (result.stdout !== "" || result.stderr !== "") {
    throw new Error(`workload produced unexpected output\nstdout: ${result.stdout}\nstderr: ${result.stderr}`);
  }
}

function writeString(buffer, offset, length, text) {
  const encoded = Buffer.from(text);
  if (encoded.length > length) throw new Error(`tar field is too long: ${text}`);
  encoded.copy(buffer, offset);
}

function writeOctal(buffer, offset, length, value) {
  writeString(buffer, offset, length, `${value.toString(8).padStart(length - 1, "0")}\0`);
}

function tarEntry(name, contents) {
  const header = Buffer.alloc(512);
  writeString(header, 0, 100, name);
  writeOctal(header, 100, 8, 0o644);
  writeOctal(header, 108, 8, 0);
  writeOctal(header, 116, 8, 0);
  writeOctal(header, 124, 12, contents.length);
  writeOctal(header, 136, 12, 0);
  header.fill(0x20, 148, 156);
  header[156] = "0".charCodeAt(0);
  writeString(header, 257, 6, "ustar\0");
  writeString(header, 263, 2, "00");
  const checksum = header.reduce((sum, byte) => sum + byte, 0);
  writeString(header, 148, 8, `${checksum.toString(8).padStart(6, "0")}\0 `);
  const padding = Buffer.alloc((512 - (contents.length % 512)) % 512);
  return Buffer.concat([header, contents, padding]);
}

function archiveFixture() {
  const entries = [];
  for (let index = 0; index < 128; index += 1) {
    const name = `data/record-${index.toString().padStart(3, "0")}.txt`;
    const contents = Buffer.from(`${index}:Nocter native workload\n`.repeat((index % 17) + 1));
    entries.push(tarEntry(name, contents));
  }
  const tar = Buffer.concat([...entries, Buffer.alloc(1024)]);
  return zlib.gzipSync(tar, { level: 6, mtime: 0 });
}

function lineFrequencyFixture() {
  const values = [];
  for (let index = 0; index < 32_768; index += 1) {
    if (index % 4 === 0) values.push("alpha");
    else if (index % 4 === 1) values.push("beta");
    else if (index % 4 === 2) values.push("gamma");
    else values.push(`item-${index % 257}`);
  }
  return Buffer.from(`${values.join("\n")}\n`);
}

function jsonFixture() {
  const records = [];
  for (let index = 0; index < 4_096; index += 1) {
    records.push({
      id: index,
      name: `record-${index}`,
      enabled: index % 3 !== 0,
      values: [index, index + 1, index + 2, null],
    });
  }
  return Buffer.from(`${JSON.stringify({ records, version: 1 }, null, 2)}\n`);
}

function inputIdentity(files) {
  return files.map((file) => ({ name: file.name, bytes: file.contents.length, sha256: digest(file.contents) }));
}

const binaryRecord = Buffer.from(
  "4e43545200010c2a000000400c00000000000067ee5d6f" +
    "4e43545200010c07000000bff400000000000090aaf521",
  "hex",
);

const lineInput = lineFrequencyFixture();
const jsonInput = jsonFixture();
const archiveInput = archiveFixture();

const WORKLOADS = [
  {
    name: "line_frequency",
    source: "examples/line-frequency",
    categories: ["synchronous", "allocation", "collections", "text", "filesystem"],
    inputs: inputIdentity([{ name: "lines.txt", contents: lineInput }]),
    prepare(root) {
      fs.writeFileSync(path.join(root, "lines.txt"), lineInput);
    },
    reset() {},
    invocation(root) {
      return textInvocation(root, ["lines.txt", "alpha"], (result) => {
        const expected = "lines: 32768\nunique: 260\nmatching: 8192\n";
        if (result.stdout !== expected || result.stderr !== "") {
          throw new Error("line-frequency output changed");
        }
      });
    },
  },
  {
    name: "json_normalize",
    source: "examples/json-normalize",
    categories: ["synchronous", "fallible", "allocation", "parsing", "filesystem"],
    inputs: inputIdentity([{ name: "input.json", contents: jsonInput }]),
    prepare(root) {
      fs.writeFileSync(path.join(root, "input.json"), jsonInput);
    },
    reset() {},
    invocation(root) {
      return textInvocation(root, ["input.json"], (result) => {
        if (result.stderr !== "" || JSON.parse(result.stdout).records.length !== 4_096) {
          throw new Error("json-normalize output changed");
        }
      });
    },
  },
  {
    name: "archive_inspect",
    source: "examples/archive-inspect",
    categories: ["synchronous", "fallible", "allocation", "parsing", "compression", "filesystem"],
    inputs: inputIdentity([{ name: "sample.tar.gz", contents: archiveInput }]),
    prepare(root) {
      fs.writeFileSync(path.join(root, "sample.tar.gz"), archiveInput);
    },
    reset() {},
    invocation(root) {
      return textInvocation(root, ["sample.tar.gz"], (result) => {
        const lines = result.stdout.trimEnd().split("\n");
        if (
          result.stderr !== "" ||
          lines.length !== 128 ||
          !lines[0].endsWith("data/record-000.txt") ||
          !lines[127].endsWith("data/record-127.txt")
        ) {
          throw new Error("archive-inspect output changed");
        }
      });
    },
  },
  {
    name: "binary_record",
    source: "examples/binary-record",
    categories: ["fallible", "asynchronous", "parsing", "filesystem"],
    inputs: [],
    prepare() {},
    reset(root) {
      remove(root, ["records.bin"]);
    },
    invocation(root) {
      return textInvocation(root, ["records.bin"], (result) => {
        quiet(result);
        if (!fs.readFileSync(path.join(root, "records.bin")).equals(binaryRecord)) {
          throw new Error("binary-record output changed");
        }
      });
    },
  },
  {
    name: "subprocess_pipeline",
    source: "examples/subprocess-pipeline",
    categories: ["fallible", "asynchronous", "cancellation", "process"],
    inputs: [],
    prepare() {},
    reset() {},
    invocation(_root, sourceRoot) {
      return textInvocation(sourceRoot, [], quiet);
    },
  },
  {
    name: "async_http",
    source: "examples/async-http",
    categories: ["fallible", "asynchronous", "cancellation", "networking", "http"],
    inputs: [],
    prepare() {},
    reset() {},
    invocation(root) {
      return textInvocation(root, [], quiet);
    },
  },
  {
    name: "http_service",
    source: "examples/http-service",
    categories: ["fallible", "allocation", "asynchronous", "cancellation", "filesystem", "networking", "http"],
    inputs: [],
    prepare() {},
    reset(root) {
      remove(root, [".http-service-ready", ".http-service-state", ".http-service-state.tmp"]);
    },
    invocation(root) {
      return textInvocation(root, [], quiet);
    },
  },
];

function nativeWorkloads(repository) {
  return WORKLOADS.map((workload) => ({
    ...workload,
    sourceRoot: path.join(repository, workload.source),
    sources: identities(repository, path.join(repository, workload.source)),
  }));
}

module.exports = { nativeWorkloads };
