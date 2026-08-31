# Development Milestones

This directory contains the active development milestone, completed milestones, and historical
milestone records.
Public language behavior belongs in [`spec/`](../../spec/README.md); published qualification belongs
in [`development/releases/`](../releases/README.md).

## Active Milestone

The [v0.24.0 milestone](v0.24.0.md) makes imported value ownership explicit through module-qualified
use sites, reserves selected imports for type names, and requires `UPPER_SNAKE_CASE` constants.
Implementation, source migration, qualification, and final review are complete; release preparation
has not started.

## Completed Unpublished Milestone

The [v0.23.0 milestone](v0.23.0.md) completed type-owned decimal parsing and owned text conversion
for every built-in integer. Its implementation and full design review are complete; v0.24.0
superseded it before separate release preparation began.

## Latest Published Milestones

The [v0.22.0 milestone](v0.22.0.md) adds an owning JSON DOM, exact decimal tokens, strict parsing,
compact generation, practical application integration, and standard-library stabilization. Phases
0 through 5 and release qualification are complete. The release is published and externally
audited in the [v0.22.0 publication record](../releases/v0.22.0.md).

The [v0.21.0 milestone](v0.21.0.md) returns to practical standard-library work with
representation-neutral `Map` and `Set` contracts, a private hash-table implementation, and the
language and target prerequisites needed to implement them in ordinary Nocter source. Phase 0
through Phase 5 and release qualification are complete; its
[release preparation](v0.21.0-release-preparation.md) owns the frozen candidate identity and
qualification evidence. The release is published and externally audited in the
[v0.21.0 publication record](../releases/v0.21.0.md).

## Earlier Milestones

The [v0.20.0 milestone](v0.20.0.md) completed interface prerequisites, incremental semantic
computation, unified CLI/LSP query entry, and dependency-local exact selections. It remains an
unpublished compiler-foundation record rather than the active work plan.

The [v0.19.0 milestone](v0.19.0.md) and its
[release preparation](v0.19.0-release-preparation.md) are complete, and v0.19.0 is published.
The [v0.18.0 milestone](v0.18.0.md) and its
[release preparation](v0.18.0-release-preparation.md) are complete, and v0.18.0 is published.
The [v0.17.0 milestone](v0.17.0.md) and its
[release preparation](v0.17.0-release-preparation.md) are complete, and v0.17.0 is published. The
[v0.16.0 milestone](v0.16.0.md) and its
[release preparation](v0.16.0-release-preparation.md) are complete, and v0.16.0 is published. The
[v0.15.0 milestone](v0.15.0.md) and its
[release preparation](v0.15.0-release-preparation.md) are complete, and v0.15.0 is published. The
[v0.14.0 milestone](v0.14.0.md), its
[implementation qualification](v0.14.0-qualification.md), and the
[final design review](../reviews/v0.14.0-final-design.md) are complete. Earlier v0.14.0 internal
refactoring was superseded before publication by a specification-first compiler rewrite. The
[release preparation](v0.14.0-release-preparation.md) is complete.

Every milestone before v0.22.0 records past work only. Historical records are not normative
language sources and must not be used to reconstruct behavior missing from the current
specification.
