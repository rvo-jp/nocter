# Performance Measurement Boundary

Nocter treats performance evidence as an observation of public work, not as a second compiler
pipeline. The benchmark runner owns process orchestration and statistical summaries. Compiler
queries own deterministic computation accounting. Neither authority may infer the other's facts.

## Boundary

The external runner may know only documented CLI and LSP requests, representative checked source,
process exit status, protocol responses, elapsed time, and host resource observations. It may not
construct compiler semantic values, bypass installation integrity, call private crates, or encode
an internal phase order.

The computation kernel may count query executions and reuse because it owns those events. Tests
may assert that an edit invalidates only the required queries. The kernel does not own wall-clock
thresholds, host metadata, benchmark scenarios, or release comparisons.

This separation produces two complementary answers:

1. External measurements show whether an ordinary command or editor interaction improved.
2. Deterministic query tests show whether the computation model avoided unnecessary semantic work.

A timing change without a query-count change points toward implementation cost, process startup,
I/O, or code generation. A query-count regression is structural even when a fast machine hides it.

## Measurement Rules

- Compare released and candidate installed homes in the same invocation and alternate their sample
  order.
- Record compiler digests, host facts, repository revision, scenario identity, and sample counts.
- Use medians for the primary comparison and retain distribution summaries rather than one run.
- Keep correctness and diagnostic expectations in conformance tests. A benchmark fails only when
  its scenario cannot execute faithfully.
- Do not persist an incremental build cache or weaken installation validation to make a number
  smaller.
- Attribute a measured bottleneck before changing architecture. Internal instrumentation requires
  its own explicit observation contract and must remain removable from semantic responsibilities.

Performance results are milestone evidence, not a stable promise about absolute duration on other
machines.
