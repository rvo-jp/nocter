# nocter-native-session

## Responsibility

Compose a successful compiler session with Target, MIR, Machine, ARM64, and Mach-O lowering and
return a complete native artifact or typed failure.

## Contract

The crate orchestrates existing stage contracts and owns output bytes plus native test execution
support. It does not implement source-language checks, backend rules, command-line policy, or
filesystem publication transactions.

## Invariants

- Each stage consumes exactly the previous closed product.
- A backend integrity failure cannot be presented as a source-language diagnostic.
- Partial native output is never returned as a successful artifact.
