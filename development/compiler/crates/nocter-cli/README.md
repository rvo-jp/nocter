# nocter-cli

## Responsibility

Own the `nocter` process boundary: host arguments and environment, command dispatch, LSP process
entry, human/JSON reporting, exit status, and native test reporting.

## Contract

The crate converts process inputs into `nocter-command` or language-server requests and projects
their typed outcomes to stdout, stderr, and process status. It does not implement command planning,
compiler semantics, package resolution, or LSP feature logic.

## Internal Responsibilities

- exhaustive classification into direct commands and installation-dependent commands
- direct source tooling and package initialization execution
- installed toolchain validation and typed command dispatch
- process result and diagnostic presentation

## Invariants

- Process I/O and environment access remain at this outer boundary.
- Rendering cannot change command success or diagnostic identity.
- A parsed command crosses exactly one exhaustive route boundary. Installed dispatch cannot
  represent help, initialization, formatting, or source inspection, so no unreachable branch is
  required to defend an earlier classification.
- LSP transport and ordinary command execution use distinct explicit entry paths.
