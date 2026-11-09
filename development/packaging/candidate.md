# Current Release Candidate

## Readiness

The retained v0.73.0 arm64-darwin candidate is qualified and awaits explicit publication
authorization. Qualification belongs to the exact source and artifact identities below, not the
current development HEAD. Do not rebuild it or replace it as part of v2.0.0 work.

| Identity | Qualified value |
| --- | --- |
| Release-content commit | `a42b1fb523a9103f45b400e0172c17e473d07929` |
| Retained archive | `development/dist/nocter-v0.73.0-arm64-darwin.tar.gz` |
| Archive bytes | 9,609,573 |
| Archive SHA-256 | `2b30313c5a9b98cb43374df14ff81ce61a0ce0006cd4c8d10967f999950e5a78` |
| Compiler SHA-256 | `a11439bddf2a3a3a1e545f6adcb55832265255fa52b838a067a451e79a0e15d1` |
| Standard-library tree SHA-256 | `fa1ee68b6e2f7c6887662c0883a4aeed99659efd6bb9c688d300b5baafaf367f` |
| Standard-library regular files | 479 |

[Packaging](README.md) owns current assembly and installed-artifact qualification requirements.
Source input identity belongs to VERSION and RELEASE.json; this page identifies the one retained
qualified output. It is not a qualification claim for later source changes.

## Remaining Publication Work

After authorization, publish the exact retained archive with a tag bound to the recorded
release-content commit. Verify the remote asset, installed home, release endpoint, workflows and
source-identified documentation deployment independently. Preserve published tags and artifacts;
a correction needs a new release identity. Remove this candidate state when it no longer
represents pending work; Git retains its earlier form.
