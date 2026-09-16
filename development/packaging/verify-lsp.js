#!/usr/bin/env node
"use strict";

const fs = require("node:fs");
const path = require("node:path");
const { pathToFileURL } = require("node:url");
const { editorClientCapabilities, LspClient } = require("../verification/lsp-client");

function fail(message) {
  throw new Error(`packaged LSP verification failed: ${message}`);
}

function positionOf(source, needle, characterOffset = 0) {
  const offset = source.indexOf(needle);
  if (offset < 0) fail(`source does not contain ${JSON.stringify(needle)}`);
  const lines = source.slice(0, offset + characterOffset).split("\n");
  return { line: lines.length - 1, character: lines[lines.length - 1].length };
}

function completionItems(result) {
  if (Array.isArray(result)) return result;
  if (result !== null && Array.isArray(result.items)) return result.items;
  fail(`completion result has an invalid shape: ${JSON.stringify(result)}`);
}

function assertLocations(result, suffix, feature) {
  if (!Array.isArray(result) || !result.some((location) => location.uri?.endsWith(suffix))) {
    fail(`${feature} did not resolve to ${suffix}: ${JSON.stringify(result)}`);
  }
}

async function diagnosticsForVersion(client, uri, version) {
  return client.waitForNotification(
    "textDocument/publishDiagnostics",
    (params) => params?.uri === uri && params?.version === version,
  );
}

async function verify() {
  if (process.argv.length !== 5) {
    fail("usage: verify-lsp.js NOCTER WORKSPACE VERSION");
  }

  const [binaryArgument, workspaceArgument, version] = process.argv.slice(2);
  const binary = fs.realpathSync(binaryArgument);
  const workspace = fs.realpathSync(workspaceArgument);
  const home = path.dirname(binary);
  const standardSources = [
    path.join(home, "std", "error", "index.nct"),
    path.join(home, "std", "error", "construction.nct"),
    path.join(home, "std", "internal", "unicode", "tables.nct"),
    path.join(home, "std", "str", "casing.nct"),
  ].map((sourcePath) => ({
    path: sourcePath,
    uri: pathToFileURL(sourcePath).href,
    text: fs.readFileSync(sourcePath, "utf8"),
  }));
  const userPath = path.join(workspace, "index.nct");
  const userUri = pathToFileURL(userPath).href;
  const userText = fs.readFileSync(userPath, "utf8");
  const workspaceUri = pathToFileURL(workspace).href;
  const client = new LspClient(binary, workspace);
  let stopped = false;

  try {
    const initialization = await client.request("initialize", {
      processId: process.pid,
      clientInfo: { name: "Nocter release qualification", version: "1" },
      rootUri: workspaceUri,
      workspaceFolders: [{ uri: workspaceUri, name: path.basename(workspace) }],
      capabilities: editorClientCapabilities(),
    });
    if (initialization?.serverInfo?.name !== "Nocter") {
      fail("initialize response has the wrong server name");
    }
    if (initialization.serverInfo.version !== version) {
      fail(`initialize reported ${initialization.serverInfo.version}, expected ${version}`);
    }
    const capabilities = initialization.capabilities;
    for (const name of [
      "hoverProvider",
      "completionProvider",
      "definitionProvider",
      "implementationProvider",
      "semanticTokensProvider",
    ]) {
      if (!capabilities?.[name]) fail(`initialize did not advertise ${name}`);
    }

    client.notify("initialized", {});
    await client.waitForServerRequest("client/registerCapability");

    for (const source of standardSources) {
      client.notify("textDocument/didOpen", {
        textDocument: {
          uri: source.uri,
          languageId: "nocter",
          version: 1,
          text: source.text,
        },
      });
      const tokens = await client.request("textDocument/semanticTokens/full", {
        textDocument: { uri: source.uri },
      });
      if (!Array.isArray(tokens?.data) || tokens.data.length === 0) {
        fail(`installed standard source did not produce semantic tokens: ${source.path}`);
      }
      const diagnostics = client.diagnosticsFor(source.uri);
      if (diagnostics.length !== 0) {
        fail(`installed standard source reported diagnostics: ${source.path}`);
      }
      client.notify("textDocument/didClose", { textDocument: { uri: source.uri } });
    }

    client.notify("textDocument/didOpen", {
      textDocument: { uri: userUri, languageId: "nocter", version: 1, text: userText },
    });
    const printPosition = positionOf(userText, "io.print", "io.".length);
    const hover = await client.request("textDocument/hover", {
      textDocument: { uri: userUri },
      position: printPosition,
    });
    if (!JSON.stringify(hover).includes("func print(text: &str): void!")) {
      fail(`hover did not describe std/io.print: ${JSON.stringify(hover)}`);
    }

    const completion = await client.request("textDocument/completion", {
      textDocument: { uri: userUri },
      position: printPosition,
    });
    if (!completionItems(completion).some((item) => item.label === "io.print")) {
      fail(`completion did not contain io.print: ${JSON.stringify(completion)}`);
    }
    if (client.diagnosticsFor(userUri).length !== 0) {
      fail(
        `initialized user source reported diagnostics: ${JSON.stringify(client.diagnosticsFor(userUri))}`,
      );
    }

    const definition = await client.request("textDocument/definition", {
      textDocument: { uri: userUri },
      position: printPosition,
    });
    assertLocations(definition, "/std/io/index.nct", "definition");
    const implementation = await client.request("textDocument/implementation", {
      textDocument: { uri: userUri },
      position: printPosition,
    });
    assertLocations(implementation, "/std/io/output.nct", "implementation");

    const invalidText = userText.replace("return 0", "return false");
    if (invalidText === userText) fail("user-source mutation no longer applies");
    client.notify("textDocument/didChange", {
      textDocument: { uri: userUri, version: 2 },
      contentChanges: [{ text: invalidText }],
    });
    let invalidDiagnostics;
    try {
      invalidDiagnostics = await diagnosticsForVersion(client, userUri, 2);
    } catch (error) {
      fail(
        `invalid user edit did not publish versioned diagnostics (${error.message}); observed ${JSON.stringify(client.recentNotifications(5))}`,
      );
    }
    if (!Array.isArray(invalidDiagnostics.diagnostics) || invalidDiagnostics.diagnostics.length === 0) {
      fail("invalid user edit did not publish diagnostics");
    }

    client.notify("textDocument/didChange", {
      textDocument: { uri: userUri, version: 3 },
      contentChanges: [{ text: userText }],
    });
    const restoredDiagnostics = await diagnosticsForVersion(client, userUri, 3);
    if (restoredDiagnostics.diagnostics?.length !== 0) {
      fail(`restored user source reported diagnostics: ${JSON.stringify(restoredDiagnostics)}`);
    }
    const restoredHover = await client.request("textDocument/hover", {
      textDocument: { uri: userUri },
      position: printPosition,
    });
    if (restoredHover === null) fail("hover did not recover after restoring the user source");

    client.notify("textDocument/didClose", { textDocument: { uri: userUri } });
    await client.stop();
    stopped = true;
  } finally {
    if (!stopped) await client.abort();
  }
}

verify().catch((error) => {
  process.stderr.write(`${error.message}\n`);
  process.exitCode = 1;
});
