# Current Development Design

## Goals

Keep ordinary code focused on the programmer's actual intent. Directory structure, contract-only
index.nct files and see hints reduce how much source must be opened; concise consistent syntax
and compiler inference reduce unnecessary writing and reasoning. Preserve information required
for safety and useful optimization, with local explanations when proof is unavailable. Evaluate
models on their meaning rather than treating Rust or another language as the correct template.
Implementation effort and short-term delivery do not limit the design. LLVM independence gives
Nocter control of its optimization pipeline; correctness and measured removal of runtime work
remain necessary before performance claims.

## Design Owners

These documents own unresolved design and qualification work, not adopted source semantics or
implementation contracts. Accepted language rules belong to the [development specification](../spec/README.md),
and implemented handoffs belong to [architecture](../architecture/README.md) and crate READMEs.
Move a resolved subject into its owner; remove completed work rather than retaining an archive.
This directory is excluded from website publication and search, but its links are validated.

- [Remaining Source Grammar](source-grammar.md): unresolved whole-source productions and parser boundaries.
- [Compiler Realization](compiler-realization.md): remaining source, proof and executable admission obligations.
- [Optimization Experiments](optimization.md): research candidates, evidence and experimental controls.

The [current work plan](../TODO.md) selects work order. The [agent instructions](../AGENTS.md)
define the delegated decision scope and required syntax approvals.
