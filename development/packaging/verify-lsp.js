#!/usr/bin/env node

const fs = require("node:fs");
const path = require("node:path");
const { spawn } = require("node:child_process");
const { pathToFileURL } = require("node:url");

function fail(message) {
  process.stderr.write(`packaged LSP verification failed: ${message}\n`);
  process.exit(1);
}

if (process.argv.length !== 5) {
  fail("usage: verify-lsp.js NOCTER WORKSPACE VERSION");
}

const [binary, workspace, version] = process.argv.slice(2);
const environment = { ...process.env };
delete environment.NOCTER_HOME;

const home = path.dirname(binary);
const standardSources = [
  path.join(home, "std", "error", "index.nct"),
  path.join(home, "std", "error", "construction.nct"),
].map((sourcePath) => ({
  path: sourcePath,
  uri: pathToFileURL(sourcePath).href,
  text: fs.readFileSync(sourcePath, "utf8"),
}));

const requests = [
  {
    jsonrpc: "2.0",
    id: 1,
    method: "initialize",
    params: { rootUri: pathToFileURL(workspace).href, capabilities: {} },
  },
  { jsonrpc: "2.0", method: "initialized", params: {} },
  ...standardSources.flatMap((source, index) => [
    {
      jsonrpc: "2.0",
      method: "textDocument/didOpen",
      params: {
        textDocument: {
          uri: source.uri,
          languageId: "nocter",
          version: 1,
          text: source.text,
        },
      },
    },
    {
      jsonrpc: "2.0",
      id: index + 2,
      method: "textDocument/semanticTokens/full",
      params: { textDocument: { uri: source.uri } },
    },
  ]),
  { jsonrpc: "2.0", id: 4, method: "shutdown" },
  { jsonrpc: "2.0", method: "exit" },
];
const input = requests
  .map((request) => {
    const body = JSON.stringify(request);
    return `Content-Length: ${Buffer.byteLength(body)}\r\n\r\n${body}`;
  })
  .join("");

const child = spawn(binary, ["lsp"], {
  cwd: workspace,
  env: environment,
  stdio: ["pipe", "pipe", "pipe"],
});
const stdout = [];
const stderr = [];
child.stdout.on("data", (chunk) => stdout.push(chunk));
child.stderr.on("data", (chunk) => stderr.push(chunk));
child.on("error", (error) => fail(error.message));
child.stdin.end(input);

child.on("close", (code, signal) => {
  if (code !== 0 || signal !== null) {
    fail(`server exited with code ${code} and signal ${signal}`);
  }
  const errorText = Buffer.concat(stderr).toString("utf8");
  if (errorText !== "") {
    fail(`server wrote to stderr: ${errorText}`);
  }

  let remaining = Buffer.concat(stdout);
  const messages = [];
  while (remaining.length > 0) {
    const boundary = remaining.indexOf("\r\n\r\n");
    if (boundary < 0) fail("response has an incomplete header");
    const header = remaining.subarray(0, boundary).toString("ascii");
    const match = /^Content-Length: (\d+)$/.exec(header);
    if (!match) fail(`response has an invalid header: ${header}`);
    const length = Number(match[1]);
    const bodyStart = boundary + 4;
    const bodyEnd = bodyStart + length;
    if (remaining.length < bodyEnd) fail("response has an incomplete body");
    messages.push(JSON.parse(remaining.subarray(bodyStart, bodyEnd).toString("utf8")));
    remaining = remaining.subarray(bodyEnd);
  }

  const initialize = messages.find((message) => message.id === 1);
  const shutdown = messages.find((message) => message.id === 4);
  if (initialize?.result?.serverInfo?.name !== "Nocter") {
    fail("initialize response has the wrong server name");
  }
  if (initialize.result.serverInfo.version !== version) {
    fail(`initialize reported ${initialize.result.serverInfo.version}, expected ${version}`);
  }
  for (const [index, source] of standardSources.entries()) {
    const tokens = messages.find((message) => message.id === index + 2);
    if (!Array.isArray(tokens?.result?.data) || tokens.result.data.length === 0) {
      fail(`installed standard source did not produce semantic tokens: ${source.path}`);
    }
    const diagnostics = messages.filter(
      (message) =>
        message.method === "textDocument/publishDiagnostics" &&
        message.params?.uri === source.uri,
    );
    if (diagnostics.some((message) => message.params.diagnostics?.length !== 0)) {
      fail(`installed standard source reported diagnostics: ${source.path}`);
    }
  }
  if (!shutdown || shutdown.result !== null) {
    fail("shutdown response is absent or invalid");
  }
});
