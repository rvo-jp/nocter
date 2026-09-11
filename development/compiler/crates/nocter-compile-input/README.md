# nocter-compile-input

## Responsibility

Own the closed, syntax-backed compilation input shared by declaration lowering, checking, and
target validation.

## Contract

The crate packages reached sources and syntax trees, package/module identity, dependency edges,
selected target facts, toolchain declaration locators, and runtime contract input into immutable
values. Primitive, trusted target-service, and runtime-storage locators name exact declarations but
contain no resolved semantic identity. The crate does not discover files or perform semantic
lowering.

## Invariants

- Every source, syntax tree, module, and dependency edge belongs to one compile unit.
- Production discovery constructs one owned input and shares its immutable source and syntax
  storage; semantic queries borrow that same input rather than rebuilding topology.
- Syntax handles can outlive a temporary input borrow. Current semantic catalogs retain those
  handles instead of copying trees or depending on a self-referential discovery owner.
- Target selection is supplied as one completed authority and is never recomputed downstream.
- Construction with a supplied target selection validates the exact target, syntax-root multiset,
  and source-map membership once. Access never repeats that traversal; a stale or independently
  assembled selection remains a typed integrity error stored by the immutable input.
- Source lookup returns a tree only when the compile unit contains exactly one tree for that source
  identity.
- Dependency identities are canonical; display names and paths cannot substitute for them.
- Directly constructed test inputs obey the same validation boundary as production inputs.
