# Declaration Diagnostic Boundary

This document owns the cross-crate failure and diagnostic contract from discovery through
declaration analysis. Public diagnostic meaning and stable codes remain owned by
[Diagnostics](../../spec/12-diagnostics.md). Exact Rust variants and crate-local adapters belong to
their defining source and rustdoc.

## Production Path

```text
DiscoveredUnit + canonical declaration surfaces
        |
        v
private declaration query
        |
        +--> accepted ReusableDeclarations
        +--> exact-current authored rejection
        `--> integrity unavailability
                    |
                    v
        closed program/unit analysis result
                    |
                    v
        session diagnostic and recovery evidence
```

`nocter-compiler-computation` is the production query owner. Eager declaration-lowering helpers
exist only for focused tests and native composition tests; command, workspace, session, analysis,
and LSP code cannot use them to establish a second failure order.

## Failure Classes

Every failure has one owner and one class:

- **source or syntax rejection**: lexing or parsing owns the diagnostic; declaration analysis does
  not manufacture a second semantic error for the same malformed syntax;
- **authored declaration rule**: declaration lowering selects a stable rule and retains exact
  semantic subjects that can be projected to source;
- **discovery contract failure**: package, module, source membership, visibility, import, target, or
  toolchain input contradicts the frozen discovery snapshot;
- **compiler integrity failure**: an identity, arena, projection recipe, or completed earlier-stage
  contract is internally inconsistent.

Only the first two classes may produce public source diagnostics. Discovery contract and compiler
integrity failures remain typed infrastructure failures; assigning them a language code would
misrepresent a compiler defect as an authored mistake.

## Rule Selection and Projection

The stage that detects an authored violation owns its rule identity and semantic subjects.
`nocter-diagnostics` owns the phase-neutral envelope and rendering, but it cannot select a rule,
repeat lookup, or infer a subject from display text.

Declaration lowering projects a complete authored report through the current source projection.
The report is ordered canonically and duplicate-free before it leaves the stage. A missing subject
or mismatched generation invalidates the whole projection; consumers cannot publish whichever
prefix happened to project successfully.

`SourceIndex` locates an already selected semantic subject. It never decides that a declaration is
invalid, changes diagnostic cardinality, or supplies a fallback rule.

## Rejection and Recovery

An authored rejection is a first-class exact-current query value. It retains its complete
diagnostic set and one branchable declaration recovery product. The top-level analysis query, not
session, continues that recovery through the deepest body/name evidence permitted by the rejected
declarations.

Recovery consumes admission facts selected during declaration validation. It cannot infer
authorization from diagnostic codes, rerun a validator, or traverse rejected declarations to
reconstruct the same decision. A declaration-only rejection cannot be converted into the input
accepted by body analysis.

Session receives one closed success or rejection branch and preserves the complete diagnostic set
in its failure envelope. CLI and LSP presentation read that same set; neither narrows it to one
primary error and rebuilds the remainder.

## Stage Ownership

| Failure domain | Sole owner |
|---|---|
| source bytes, lexing, parsing, and syntax recovery | `nocter-source` and `nocter-syntax` |
| package roots, modules, imports, visibility edges, and discovery topology | `nocter-package`, `nocter-target-selection`, and `nocter-discovery` |
| declaration contracts, headers, generics, types, definitions, and authored imports | `nocter-declaration-lowering` |
| body names, typing, capability evidence, ownership, provenance, and loans | `nocter-checking` |
| target availability, executable closure, ABI, and artifact construction | their target or backend owner |
| diagnostic envelope and human/JSON rendering | `nocter-diagnostics` |

The [grammar conformance matrix](grammar-conformance.md) assigns syntax and semantic-boundary cases
to the narrowest owning stage. A declaration query cannot reject a valid intermediate program on
behalf of checking, target validation, or a backend.

## Required Invariants

- One authored violation has one rule owner and one source projection.
- Diagnostic rendering cannot affect compilation or recovery.
- Authored reports have deterministic ordering independent of traversal order.
- A lost source subject is an integrity failure, not permission to widen a range.
- Internal failure cannot become an authored diagnostic or an empty successful result.
- Session and feature code cannot restart declaration analysis.
- CLI and LSP consume the same closed diagnostic collection.
