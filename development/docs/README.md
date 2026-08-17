# Nocter Development Documents

This directory contains the active implementation policy for the specification-first compiler
rewrite. Public language and standard-library behavior belongs exclusively in
[`spec/`](../../spec/README.md).

## Active Documents

- [Compiler Rewrite Architecture](architecture.md)
- [Checked Program Design](checked-program-design.md)
- [Target and Executable Program Design](target-program-design.md)
- [Machine Program and Native Target Design](machine-program-design.md)
- [Declaration Diagnostic Boundary](declaration-diagnostic-boundary.md)
- [Grammar Closure Audit](grammar-audit.md)
- [Grammar Conformance Plan](grammar-conformance.md)
- [Maintenance](maintenance.md)
- [Documentation Site Generation](site-generation.md)
- [Current Handoff](../TODO.md)
- [v0.14.0 Rewrite Milestone](../milestones/v0.14.0.md)

Historical implementation design was removed from the active tree when the compiler rewrite
started. Git history and published release records preserve it for archival purposes, but it must
not be consulted to determine new compiler structure or unspecified language behavior.

## Information Ownership

| Information | Sole owner |
|---|---|
| Public language and standard-library behavior | `spec/` |
| New compiler dependency and authority boundaries | `architecture.md` |
| Phase 3 checked-program responsibilities and construction order | `checked-program-design.md` |
| Phase 4 target validation, instantiation, and MIR ownership | `target-program-design.md` |
| Phase 5 machine layout, machine program, ARM64, and Mach-O ownership | `machine-program-design.md` |
| Declaration-lowering failure classification | `declaration-diagnostic-boundary.md` |
| Grammar-gate inventory and closure progress | `grammar-audit.md` |
| Parser test derivation from the grammar | `grammar-conformance.md` |
| Rewrite scope and completion gates | `../milestones/v0.14.0.md` |
| Next concrete work and blockers | `../TODO.md` |
| Published qualification evidence | `../releases/` |
| Documentation generation | `site-generation.md` |

Do not duplicate normative rules in development documents. Link to the owning specification rule.
