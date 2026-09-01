# nocter-toolchain-contract

## Responsibility

Own the closed contract describing declarations and standard-library roles that a selected Nocter
toolchain must provide.

## Contract

Discovery and declaration lowering bind exact source declarations to this contract. Checking and
target validation consume resolved role identities rather than standard-library spellings.
The allocation-effect authority uses the selected standard package's exact `AllocationRequest`
callable as its backing-storage request boundary; OS primitive names are not semantic evidence.

## Invariants

- Toolchain roles are source-independent and versioned with the compiler.
- A standard path does not grant authority by itself.
- Consumers cannot extend the closed role set through ordinary user source.
