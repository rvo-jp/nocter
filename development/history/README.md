# Development History

This directory preserves non-normative engineering records without publishing them as current
contributor guidance or website content. Public behavior is defined only by
[`spec/`](../../spec/README.md).

- [Milestones](milestones/README.md) record scope, work order, and completion gates.
- [Design Reviews](reviews/README.md) record findings and remediation evidence. The
  [v2.0.0 catalog](reviews/README.md#value-binding-reviews) separates selected design, implementation evidence,
  research, and superseded checkpoints; the current reading path starts in
  [contributor documentation](../README.md#value-binding-reading-order).
- [Release Qualification](release-audits/README.md) records immutable publication evidence.
- `legacy-design/` preserves documents for the compiler removed at the start of the v0.14.0
  specification-first rewrite.

Historical records explain why a change was made; they are not inputs for reconstructing current
semantics. Do not consult `legacy-design/` when defining behavior, designing the current compiler,
writing conformance cases, or resolving an ambiguity.
