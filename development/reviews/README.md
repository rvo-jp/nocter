# Compiler Design Reviews

This directory owns cross-cutting compiler review criteria, evidence, findings, and remediation
status. It is non-normative: public behavior remains owned exclusively by [`spec/`](../../spec/README.md).

An implementation qualification proves the recorded implementation against its specification and
tests. A final design review asks a different question: whether each responsibility can be replaced
internally without forcing consumers that use only its contract to change.

## Current Candidate Reviews

- [v0.19.0 Phase 0 filesystem traversal review](v0.19.0-phase-0.md) — complete
- [v0.19.0 Phase 1 streaming text input review](v0.19.0-phase-1.md) — complete
- [v0.18.0 Phase 0 construction review](v0.18.0-phase-0.md) — complete
- [v0.18.0 Phase 1 builtin declaration review](v0.18.0-phase-1.md) — complete
- [v0.18.0 Phase 2 interface implementation review](v0.18.0-phase-2.md) — complete
- [v0.18.0 Phase 3 persistent authority review](v0.18.0-phase-3.md) — complete
- [v0.18.0 persistent semantic authority record](v0.18.0-persistent-semantic-authority.md) — complete
- [v0.18.0 semantic tooling reconstruction record](v0.18.0-semantic-tooling-reconstruction.md) — complete

## Historical Foundation Reviews

- [v0.17.0 analysis authority reconstruction](v0.17.0-analysis-authority.md)
- [v0.14.0 final design review](v0.14.0-final-design.md)
- [v0.14.0 grammar closure audit](v0.14.0-grammar-audit.md)
