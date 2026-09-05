"use strict";

const { spawn } = require("node:child_process");

class LspClient {
  constructor(binary, cwd) {
    const environment = { ...process.env };
    delete environment.NOCTER_HOME;
    this.child = spawn(binary, ["lsp"], {
      cwd,
      env: environment,
      stdio: ["pipe", "pipe", "pipe"],
    });
    this.buffer = Buffer.alloc(0);
    this.nextId = 1;
    this.pending = new Map();
    this.notifications = [];
    this.stderr = [];
    this.closed = new Promise((resolve, reject) => {
      this.child.on("error", reject);
      this.child.on("close", (code, signal) => resolve({ code, signal }));
    });
    this.child.stderr.on("data", (chunk) => this.stderr.push(chunk));
    this.child.stdout.on("data", (chunk) => {
      try {
        this.receive(chunk);
      } catch (error) {
        for (const pending of this.pending.values()) {
          clearTimeout(pending.timeout);
          pending.reject(error);
        }
        this.pending.clear();
        this.child.kill();
      }
    });
  }

  receive(chunk) {
    this.buffer = Buffer.concat([this.buffer, chunk]);
    while (true) {
      const boundary = this.buffer.indexOf("\r\n\r\n");
      if (boundary < 0) return;
      const header = this.buffer.subarray(0, boundary).toString("ascii");
      const match = /^Content-Length: (\d+)$/i.exec(header);
      if (!match) throw new Error(`invalid LSP header: ${header}`);
      const length = Number(match[1]);
      const bodyStart = boundary + 4;
      const bodyEnd = bodyStart + length;
      if (this.buffer.length < bodyEnd) return;
      const message = JSON.parse(this.buffer.subarray(bodyStart, bodyEnd).toString("utf8"));
      this.buffer = this.buffer.subarray(bodyEnd);
      if (Object.hasOwn(message, "id")) {
        const pending = this.pending.get(message.id);
        if (!pending) {
          throw new Error(`unexpected LSP response ${message.id}: ${JSON.stringify(message)}`);
        }
        this.pending.delete(message.id);
        clearTimeout(pending.timeout);
        if (message.error) pending.reject(new Error(JSON.stringify(message.error)));
        else pending.resolve(message.result);
      } else {
        this.notifications.push(message);
      }
    }
  }

  send(message) {
    const body = JSON.stringify(message);
    this.child.stdin.write(`Content-Length: ${Buffer.byteLength(body)}\r\n\r\n${body}`);
  }

  notify(method, params) {
    this.send({ jsonrpc: "2.0", method, params });
  }

  request(method, params) {
    const id = this.nextId;
    this.nextId += 1;
    return new Promise((resolve, reject) => {
      const timeout = setTimeout(() => {
        this.pending.delete(id);
        reject(new Error(`LSP request ${method} timed out`));
      }, 30_000);
      this.pending.set(id, { resolve, reject, timeout });
      this.send({ jsonrpc: "2.0", id, method, params });
    });
  }

  diagnosticsFor(uri) {
    return this.notifications
      .filter(
        (message) =>
          message.method === "textDocument/publishDiagnostics" && message.params?.uri === uri,
      )
      .flatMap((message) => message.params.diagnostics ?? []);
  }

  async stop() {
    await this.request("shutdown");
    this.notify("exit");
    this.child.stdin.end();
    const exit = await this.closed;
    const stderr = Buffer.concat(this.stderr).toString("utf8");
    if (exit.code !== 0 || exit.signal !== null || stderr !== "") {
      throw new Error(
        `LSP server exited with code ${exit.code} and signal ${exit.signal}\n${stderr}`,
      );
    }
  }
}

module.exports = { LspClient };
