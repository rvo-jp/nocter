# Platform Expansion Notes

These unadopted planning notes were separated from the published platform specification.
They establish no supported target, archive, command option, or build profile. A selected future
contract must enter the [development specification](../../spec/README.md) before implementation.

## Expansion Notes

Additional targets require a target backend, executable writer, primitive set, and target-gated
standard-library declarations. Those declarations must remain in the same stable directory modules
used by portable code; ordinary programs must not name CPU instructions, object formats, or OS
syscall details. Cross compilation will use an explicit target option such as `--target x64-linux`.

Additional host archives may use names such as `nocter-v<version>-x64-linux.tar.gz` or
`nocter-v<version>-arm64-linux.tar.gz`, while retaining `.nocter/` as their archive root.

Future build-profile options may control optimization level, debug information, and diagnostics,
but cannot weaken language safety checks.
