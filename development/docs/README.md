# Nocter Development Documents

This directory contains the active implementation design for the Nocter compiler. Public language
and standard-library behavior belongs exclusively in
[`spec/`](../../spec/README.md).

## Active Documents

- [Compiler Rewrite Architecture](architecture.md)
- [Checked Program Design](checked-program-design.md)
- [Target and Executable Program Design](target-program-design.md)
- [Machine Program and Native Target Design](machine-program-design.md)
- [Declaration Diagnostic Boundary](declaration-diagnostic-boundary.md)
- [Semantic Presentation Design](semantic-presentation-design.md)
- [Standard Library Source Design](standard-library-source-design.md)
- [Grammar Conformance Plan](grammar-conformance.md)
- [Maintenance](maintenance.md)
- [Documentation Site Generation](site-generation.md)
- [Current Handoff](../TODO.md)
- [v0.17.0 Practical Application Foundations](../milestones/v0.17.0.md)
- [v0.16.0 Practical Failure Values](../milestones/v0.16.0.md)
- [v0.16.0 Release Preparation](../milestones/v0.16.0-release-preparation.md)

## Completed Foundation Records

- [Grammar Closure Audit](grammar-audit.md)
- [v0.14.0 Rewrite Milestone](../milestones/v0.14.0.md)
- [v0.14.0 Implementation Qualification](../milestones/v0.14.0-qualification.md)

Superseded implementation design is retained under `development/archive/` and excluded from the
generated website. It must not be consulted to determine current compiler structure or unspecified
language behavior. Git history and published release records preserve chronology.

## Information Ownership

| Information | Sole owner |
|---|---|
| Public language and standard-library behavior | `spec/` |
| Compiler dependency direction and cross-stage authority boundaries | `architecture.md` |
| Checked-program responsibilities and construction order | `checked-program-design.md` |
| Target validation, executable specialization, and MIR ownership | `target-program-design.md` |
| Machine layout, machine program, ARM64, and Mach-O ownership | `machine-program-design.md` |
| Declaration-lowering failure classification | `declaration-diagnostic-boundary.md` |
| Compiler-owned semantic rendering and editor query inputs | `semantic-presentation-design.md` |
| Standard-library contract and implementation-source separation | `standard-library-source-design.md` |
| Completed v0.17.0 Phase 0 scope and qualification | `../milestones/v0.17.0.md` |
| Grammar-gate inventory and closure progress | `grammar-audit.md` |
| Parser test derivation from the grammar | `grammar-conformance.md` |
| Completed v0.16.0 scope and release qualification | `../milestones/v0.16.0.md` and `../milestones/v0.16.0-release-preparation.md` |
| Completed v0.15.0 scope and release qualification | `../milestones/v0.15.0.md` and `../milestones/v0.15.0-release-preparation.md` |
| Completed v0.14.0 rewrite scope and qualification | `../milestones/v0.14.0.md` and `../milestones/v0.14.0-qualification.md` |
| Next concrete work and blockers | `../TODO.md` |
| Published qualification evidence | `../releases/` |
| Documentation generation | `site-generation.md` |

Do not duplicate normative rules in development documents. Link to the owning specification rule.
