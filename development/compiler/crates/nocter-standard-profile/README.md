# nocter-standard-profile

## Responsibility

Own the one physical declaration profile for the standard package bundled with this compiler.

## Contract

The crate projects one package identity into the exact prelude, builtin type, structural
attachment, standard declaration, primitive declaration, and trusted target-service locators
expected in the bundled standard source tree. It does not discover files, resolve declarations,
validate semantic contracts, or orchestrate a compiler session.

## Invariants

- Production commands, workspace analysis, and physical-standard tests consume the same complete
  profile. Synthetic primitive fixtures that mirror bundled source consume its primitive-locator
  subcatalog rather than copying those paths and names.
- Every closed primitive role has exactly one bundled source location.
- Every available target-service role has exactly one private bundled source location; symbols and
  foreign ABI remain in the runtime contract rather than this physical profile.
- A physical-standard test cannot reconstruct a partial bundled profile.
- Physical source paths and declaration spellings do not enter semantic or backend products.
