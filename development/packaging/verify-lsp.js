#!/usr/bin/env node

const { spawn } = require("node:child_process");

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

const requests = [
  { jsonrpc: "2.0", id: 1, method: "initialize", params: { capabilities: {} } },
  { jsonrpc: "2.0", method: "initialized", params: {} },
  { jsonrpc: "2.0", id: 2, method: "shutdown" },
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
  const shutdown = messages.find((message) => message.id === 2);
  if (initialize?.result?.serverInfo?.name !== "Nocter") {
    fail("initialize response has the wrong server name");
  }
  if (initialize.result.serverInfo.version !== version) {
    fail(`initialize reported ${initialize.result.serverInfo.version}, expected ${version}`);
  }
  if (!shutdown || shutdown.result !== null) {
    fail("shutdown response is absent or invalid");
  }
});

