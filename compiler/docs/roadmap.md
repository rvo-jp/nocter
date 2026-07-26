# Compiler Roadmap

This roadmap records implementation order. It does not define language
semantics or move the v0 finish line. Source-language rules live in
[../../spec](../../spec/README.md), and the fixed v0 completion gates live in
[v0-closure.md](v0-closure.md).

## Current Priority

The current priority is to make the standard-library-driven v0 subset reliable
end to end:

1. keep docs, spec, implementation status, and tests consistent
2. close accepted-but-not-buildable frontend/backend gaps with source-backed
   rejection diagnostics
3. continue aggregate ABI, ownership, and drop cleanup work
4. promote only stable std APIs into runtime support
5. keep LSP behavior tied to compiler facts

## Recommended Work Order

1. **Frontend closure audit**: enumerate parser-accepted forms and ensure each
   has resolver/typechecker facts or a stable rejection diagnostic.
2. **Buildability boundary**: stop unsupported runtime forms before IR/backend
   emission with diagnostics written in source-language terms.
3. **Aggregate ABI and ownership**: continue direct/indirect aggregate edges,
   field-level state, enum payload facts, aggregate cleanup, and drop glue.
4. **Std runtime**: keep `.nocter/std` aligned with `spec/11`; promote `env`,
   richer `Vec`, file, path, or allocator APIs only when their runtime contract
   is stable.
5. **Control-flow and collection promotion**: broaden loops, match payloads,
   arrays, view iteration, and generic methods only with closure-matrix and test
   updates.
6. **Release hardening**: run the full closure suite, remove stale docs, and
   make diagnostics and LSP presentation coherent.

## Constraints To Preserve

- Keep the compiler self-contained. Do not switch the current compiler line to
  LLVM or an external linker.
- Keep safety checks always enabled. Optimizers may remove checks only when they
  prove the trap condition impossible.
- Keep Nocter home deterministic: use `NOCTER_HOME` or the compiler executable's
  parent; do not silently search unrelated directories.
- Keep target-dependent std declarations inside stable std files behind
  `#target(...)`.
- Keep `interface` contract-only in v0. Do not introduce interface dispatch,
  bounds, or code reuse as a convenience for implementation.
- Keep bare string interpolation rejected by buildability until an explicit
  allocator source is designed.
- Keep LSP semantic tokens, hovers, definitions, references, symbols, and
  completions backed by resolver/typechecker facts.

## Near-Term Open Work

- Update stale references whenever syntax changes remove old spellings.
- Broaden copy-aggregate and aggregate-slice runtime support only when ABI,
  ownership, and std tests agree.
- Promote non-copy `Vec<T>` storage only after per-element drop behavior is
  designed and tested.
- Promote payload enum runtime only after match/if-is payload binding facts and
  drop cleanup are stable.
- Promote `std/process.env` only after nested fallible/optional return lowering
  and process-context storage are ready.
