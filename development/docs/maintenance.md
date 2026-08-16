# Maintenance

## Authority

`spec/` is the sole source for public behavior. Development documents describe implementation
boundaries and work order without restating language rules. Historical compiler source,
implementation documents, tests, binaries, and observed behavior are not rewrite inputs.

## Resolving Specification Gaps

When two observable behaviors are consistent with the current text:

1. stop implementation in the affected area
2. cite the incomplete or conflicting specification sections
3. write a minimal source example that distinguishes the alternatives
4. compare concrete consequences and recommend one choice
5. ask the user to decide
6. update the owning specification chapter in English
7. derive conformance cases from the adopted rule

Do not preserve unresolved alternatives as compiler modes or fallback behavior.

## Commits

- Commit specification decisions separately from their implementation when practical.
- Keep one authority replacement coherent; do not commit a new authority while retaining an
  undocumented old path.
- Do not use passing tests as the only phase-completion evidence.
- Run an adversarial audit for duplicate producers, reverse lookup, order dependence, and hidden
  compatibility before marking a phase complete.

## Verification During Specification Closure

```sh
node docs/build-docs.js
git diff --check
```

The compiler workspace will define focused and complete verification commands after the grammar
gate permits implementation. Active commands belong in `development/compiler/README.md`.

## Documentation

Public-facing documentation is written in English. Edit source Markdown and regenerate generated
HTML with `node docs/build-docs.js`. Root documentation remains user-facing; compiler-development
material remains under `development/`.
