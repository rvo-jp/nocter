# Nocter v0.4.0 Handoff

## Baseline

- branch: `develop`
- released baseline: v0.3.0
- completed milestone: v0.4.0 Phase 0 — Source-Native Package Roots and Executable Targets
- target: `arm64-darwin`

The normative implementation plan is [v0.4.0.md](docs/v0.4.0.md). Package/compiler boundaries are
defined in [package-roots.md](docs/package-roots.md). Public behavior belongs in `spec/`.

## Implemented Scope

- package-header AST, parser, formatter, and JSON AST for declarative directive data
- compiler-owned package, module, and executable identities
- `index.nct` package loading with `#name`, `#version`, and repeatable `#executable`
- exact validation for package-directive placement, fields, names, and logical module paths
- package-default `check`, `build`, and `run`; explicit `--root`, `--executable`, and file mode
- removal of implicit `main.nct` and bare-source command discovery
- public namespace re-export through `pub use path`
- `#target: "..."` declaration directive spelling
- LSP semantic ranges and go to definition for executable module values

## Phase 0 Qualification

- repository verification and warnings-denied Clippy passed
- public documentation was regenerated and verified
- optimized local distribution passed `doctor` and package check/build/run
- generated ARM64 Mach-O execution and archive inspection passed

No v0.4.0 Phase 0 work remains. Define the next phase contract before beginning dependency
resolution or another package capability.
