"use strict";

const fs = require("node:fs");
const path = require("node:path");
const { pathToFileURL } = require("node:url");
const { LspClient } = require("./lsp-client");

function positionOf(source, needle) {
  const offset = source.indexOf(needle);
  if (offset < 0) throw new Error(`benchmark source does not contain ${JSON.stringify(needle)}`);
  const prefix = source.slice(0, offset);
  const lines = prefix.split("\n");
  return { line: lines.length - 1, character: lines[lines.length - 1].length };
}

async function timedRequest(client, method, params) {
  const started = process.hrtime.bigint();
  const result = await client.request(method, params);
  return {
    elapsedMs: Number(process.hrtime.bigint() - started) / 1_000_000,
    result,
  };
}

async function runLspSession(binary, repository, warmups, edits) {
  const project = path.join(repository, "examples", "text-banner");
  const sourcePath = path.join(project, "banner.nct");
  const uri = pathToFileURL(sourcePath).href;
  const original = fs.readFileSync(sourcePath, "utf8");
  const changed = original.replace("usage: text-banner TEXT", "usage: text-banner VALUE");
  if (changed === original) throw new Error("the LSP benchmark mutation no longer applies");
  const hover = positionOf(original, "normalized");
  const client = new LspClient(binary, project);

  const initialization = await timedRequest(client, "initialize", {
    rootUri: pathToFileURL(project).href,
    capabilities: {},
  });
  client.notify("initialized", {});
  client.notify("textDocument/didOpen", {
    textDocument: { uri, languageId: "nocter", version: 1, text: original },
  });
  const opened = await timedRequest(client, "textDocument/hover", {
    textDocument: { uri },
    position: hover,
  });
  if (opened.result === null) throw new Error("the LSP benchmark hover target has no result");

  const samples = [];
  const totalEdits = warmups + edits;
  for (let index = 0; index < totalEdits; index += 1) {
    const text = index % 2 === 0 ? changed : original;
    client.notify("textDocument/didChange", {
      textDocument: { uri, version: index + 2 },
      contentChanges: [{ text }],
    });
    const response = await timedRequest(client, "textDocument/hover", {
      textDocument: { uri },
      position: hover,
    });
    if (response.result === null) throw new Error("hover disappeared after benchmark edit");
    if (index >= warmups) samples.push(response.elapsedMs);
  }

  const diagnostics = client.diagnosticsFor(uri);
  client.notify("textDocument/didClose", { textDocument: { uri } });
  await client.stop();
  if (diagnostics.length !== 0) {
    throw new Error(`the LSP benchmark source reported diagnostics: ${JSON.stringify(diagnostics)}`);
  }
  return {
    initialize_ms: initialization.elapsedMs,
    open_and_hover_ms: opened.elapsedMs,
    edit_and_hover_ms: samples,
  };
}

module.exports = { runLspSession };
