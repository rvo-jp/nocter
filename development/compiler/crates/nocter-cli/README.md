# nocter-cli

## Responsibility

Own the `nocter` process boundary: host arguments and environment, command dispatch, LSP process
entry, human/JSON reporting, exit status, and native test reporting.

## Contract

The crate converts process inputs into `nocter-command` or language-server requests and projects
their typed outcomes to stdout, stderr, and process status. It does not implement command planning,
compiler semantics, package resolution, or LSP feature logic.

## Invariants

- Process I/O and environment access remain at this outer boundary.
- Rendering cannot change command success or diagnostic identity.
- LSP transport and ordinary command execution use distinct explicit entry paths.
