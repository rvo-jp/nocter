"use strict";

const { spawn } = require("node:child_process");
const path = require("node:path");

const DEFAULT_TIMEOUT_MS = 30_000;

function editorClientCapabilities() {
  return {
    workspace: {
      workspaceFolders: true,
      didChangeWatchedFiles: { dynamicRegistration: true },
    },
    textDocument: {
      synchronization: { didSave: true },
      hover: { contentFormat: ["markdown", "plaintext"] },
      completion: { completionItem: { snippetSupport: true } },
      definition: { linkSupport: true },
    },
  };
}

class LspClient {
  constructor(binary, cwd, options = {}) {
    const environment = { ...process.env };
    delete environment.NOCTER_HOME;
    this.child = spawn(path.resolve(binary), ["lsp"], {
      cwd,
      env: environment,
      stdio: ["pipe", "pipe", "pipe"],
    });
    this.buffer = Buffer.alloc(0);
    this.nextId = 1;
    this.pending = new Map();
    this.notifications = [];
    this.notificationWaiters = [];
    this.serverRequests = [];
    this.serverRequestWaiters = [];
    this.serverRequestHandlers = new Map([
      ["client/registerCapability", () => null],
      ...Object.entries(options.serverRequestHandlers ?? {}),
    ]);
    this.timeoutMs = options.timeoutMs ?? DEFAULT_TIMEOUT_MS;
    this.stderr = [];
    this.failure = null;
    this.closed = new Promise((resolve, reject) => {
      this.child.on("error", (error) => {
        this.fail(error);
        reject(error);
      });
      this.child.on("close", (code, signal) => {
        if (
          this.pending.size > 0 ||
          this.notificationWaiters.length > 0 ||
          this.serverRequestWaiters.length > 0
        ) {
          this.fail(new Error(`LSP server closed with code ${code} and signal ${signal}`));
        }
        resolve({ code, signal });
      });
    });
    this.child.stderr.on("data", (chunk) => this.stderr.push(chunk));
    this.child.stdout.on("data", (chunk) => {
      try {
        this.receive(chunk);
      } catch (error) {
        this.fail(error);
        this.child.kill();
      }
    });
  }

  fail(error) {
    if (this.failure === null) this.failure = error;
    for (const pending of this.pending.values()) {
      clearTimeout(pending.timeout);
      pending.reject(error);
    }
    this.pending.clear();
    for (const waiter of this.notificationWaiters) {
      clearTimeout(waiter.timeout);
      waiter.reject(error);
    }
    this.notificationWaiters = [];
    for (const waiter of this.serverRequestWaiters) {
      clearTimeout(waiter.timeout);
      waiter.reject(error);
    }
    this.serverRequestWaiters = [];
  }

  receive(chunk) {
    this.buffer = Buffer.concat([this.buffer, chunk]);
    while (true) {
      const boundary = this.buffer.indexOf("\r\n\r\n");
      if (boundary < 0) return;
      const header = this.buffer.subarray(0, boundary).toString("ascii");
      const lengths = header
        .split("\r\n")
        .map((line) => /^Content-Length:\s*(\d+)$/i.exec(line))
        .filter((match) => match !== null);
      if (lengths.length !== 1) throw new Error(`invalid LSP header: ${header}`);
      const length = Number(lengths[0][1]);
      const bodyStart = boundary + 4;
      const bodyEnd = bodyStart + length;
      if (this.buffer.length < bodyEnd) return;
      const message = JSON.parse(this.buffer.subarray(bodyStart, bodyEnd).toString("utf8"));
      this.buffer = this.buffer.subarray(bodyEnd);
      this.dispatch(message);
    }
  }

  dispatch(message) {
    if (typeof message.method === "string") {
      if (Object.hasOwn(message, "id")) this.handleServerRequest(message);
      else this.handleNotification(message);
      return;
    }
    if (!Object.hasOwn(message, "id")) {
      throw new Error(`invalid LSP message: ${JSON.stringify(message)}`);
    }
    const pending = this.pending.get(message.id);
    if (!pending) {
      throw new Error(`unexpected LSP response ${message.id}: ${JSON.stringify(message)}`);
    }
    this.pending.delete(message.id);
    clearTimeout(pending.timeout);
    if (message.error) pending.reject(new Error(JSON.stringify(message.error)));
    else pending.resolve(message.result);
  }

  handleServerRequest(message) {
    const handler = this.serverRequestHandlers.get(message.method);
    if (!handler) {
      this.send({
        jsonrpc: "2.0",
        id: message.id,
        error: { code: -32601, message: `unsupported client method ${message.method}` },
      });
      this.resolveServerRequestWaiter(message);
      return;
    }
    Promise.resolve()
      .then(() => handler(message.params))
      .then(
        (result) => {
          this.send({ jsonrpc: "2.0", id: message.id, result });
          this.resolveServerRequestWaiter(message);
        },
        (error) => {
          this.send({
            jsonrpc: "2.0",
            id: message.id,
            error: { code: -32603, message: String(error) },
          });
          this.resolveServerRequestWaiter(message);
        },
      )
      .catch((error) => {
        this.fail(error);
        this.child.kill();
      });
  }

  resolveServerRequestWaiter(message) {
    this.serverRequests.push(message);
    for (let index = 0; index < this.serverRequestWaiters.length; index += 1) {
      const waiter = this.serverRequestWaiters[index];
      if (waiter.method !== message.method || !waiter.predicate(message.params)) continue;
      this.serverRequestWaiters.splice(index, 1);
      clearTimeout(waiter.timeout);
      waiter.resolve(message.params);
      return;
    }
  }

  handleNotification(message) {
    this.notifications.push(message);
    for (let index = 0; index < this.notificationWaiters.length; index += 1) {
      const waiter = this.notificationWaiters[index];
      if (waiter.method !== message.method || !waiter.predicate(message.params)) continue;
      this.notificationWaiters.splice(index, 1);
      clearTimeout(waiter.timeout);
      waiter.resolve(message.params);
      return;
    }
  }

  send(message) {
    if (this.failure !== null) throw this.failure;
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
      }, this.timeoutMs);
      this.pending.set(id, { resolve, reject, timeout });
      this.send({ jsonrpc: "2.0", id, method, params });
    });
  }

  waitForNotification(method, predicate = () => true) {
    const existing = this.notifications.find(
      (message) => message.method === method && predicate(message.params),
    );
    if (existing) return Promise.resolve(existing.params);
    return new Promise((resolve, reject) => {
      const waiter = { method, predicate, resolve, reject, timeout: null };
      waiter.timeout = setTimeout(() => {
        const index = this.notificationWaiters.indexOf(waiter);
        if (index >= 0) this.notificationWaiters.splice(index, 1);
        reject(new Error(`LSP notification ${method} timed out`));
      }, this.timeoutMs);
      this.notificationWaiters.push(waiter);
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

  recentNotifications(limit) {
    return this.notifications.slice(-limit);
  }

  waitForServerRequest(method, predicate = () => true) {
    const existing = this.serverRequests.find(
      (message) => message.method === method && predicate(message.params),
    );
    if (existing) return Promise.resolve(existing.params);
    return new Promise((resolve, reject) => {
      const waiter = { method, predicate, resolve, reject, timeout: null };
      waiter.timeout = setTimeout(() => {
        const index = this.serverRequestWaiters.indexOf(waiter);
        if (index >= 0) this.serverRequestWaiters.splice(index, 1);
        reject(new Error(`LSP server request ${method} timed out`));
      }, this.timeoutMs);
      this.serverRequestWaiters.push(waiter);
    });
  }

  async stop() {
    await this.request("shutdown");
    this.notify("exit");
    this.child.stdin.end();
    const exit = await this.closed;
    const stderr = Buffer.concat(this.stderr).toString("utf8");
    if (this.failure !== null) throw this.failure;
    if (exit.code !== 0 || exit.signal !== null || stderr !== "") {
      throw new Error(
        `LSP server exited with code ${exit.code} and signal ${exit.signal}\n${stderr}`,
      );
    }
  }

  async abort() {
    this.child.stdin.destroy();
    if (this.child.exitCode === null && this.child.signalCode === null) this.child.kill();
    await this.closed;
  }
}

module.exports = { editorClientCapabilities, LspClient };
