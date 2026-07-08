# Nocter VS Code Extension

This extension provides `.nct` file association, TextMate highlighting, editor language configuration, snippets, and a compiler-backed LSP client.

The extension does not implement Nocter semantics. Diagnostics come from `nocter lsp`.

## Development

```sh
npm install
npm run compile
```

To run the extension from this repository:

1. Open `tools/vscode-nocter/` in VS Code.
2. Select `Run Nocter Extension` in Run and Debug.
3. Start debugging. The pre-launch task builds `../../compiler` and compiles the extension.
4. In the Extension Development Host window, open this repository or any folder containing `.nct` files.
5. Open a `.nct` file and check the Problems panel.

In repository development, the extension first tries:

```text
../../compiler/target/debug/nocter
```

If `../../.nocter/` exists, the extension also sets `NOCTER_HOME` for the language server unless `NOCTER_HOME` is already set in the environment.

Outside repository development, the extension starts:

```sh
nocter lsp
```

Set `nocter.server.path` when the compiler binary is not on `PATH`.
Set `nocter.server.env.NOCTER_HOME` when the compiler binary is outside a Nocter home directory.
