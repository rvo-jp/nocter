# nocter-native-session

## Responsibility

Compose a successful compiler session with Target, MIR, Machine, ARM64, and Mach-O lowering and
return a complete native artifact or typed failure.

## Contract

The crate accepts only a query-backed `CompiledTarget`, orchestrates backend stage contracts, and
owns output bytes plus native test execution support. It does not discover source, run semantic
analysis, implement backend rules, select command-line policy, or publish filesystem artifacts.

## Invariants

- Each stage consumes exactly the previous closed product.
- Native requests cannot carry a discovery snapshot or reopen semantic compilation.
- A backend integrity failure cannot be presented as a source-language diagnostic.
- Partial native output is never returned as a successful artifact.
