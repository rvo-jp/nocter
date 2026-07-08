# Nocter VS Code Extension

This extension provides `.nct` file association, TextMate highlighting, editor language configuration, snippets, and a compiler-backed LSP client.

The extension does not implement Nocter semantics. Diagnostics come from `nocter lsp`.

## Development

```sh
npm install
npm run compile
```

By default the extension starts:

```sh
nocter lsp
```

Set `nocter.server.path` when the compiler binary is not on `PATH`.
