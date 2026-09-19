# Nocter Examples

These runnable programs demonstrate current Nocter source style and public standard-library use.
The smallest example uses explicit single-file mode; the larger example uses package mode and a
directory module split across physical sources.

Examples illustrate the qualified compiler candidate in the current repository. For exact behavior
of a published release, use the examples from that release's repository tag.

## Single-File Examples

[hello.nct](hello.nct) is the smallest executable example. Passing its filename explicitly selects
single-file mode without a package declaration or inferred entry filename.

```sh
nocter check examples/hello.nct
nocter run examples/hello.nct
```

[ownership.nct](ownership.nct) mutates aggregate elements through a readwrite borrow, observes
them through a readonly borrow, then consumes the owning `Vec`. The same concise program appears
in the website hero because it is compiled and executed as an ordinary public example.

```sh
nocter check examples/ownership.nct
nocter run examples/ownership.nct
```

[async.nct](async.nct) starts two delayed computations, awaits their structured join, and verifies
the combined result. It demonstrates that `async` functions produce ordinary `future T` values
while `await` remains the explicit extraction point.

```sh
nocter check examples/async.nct
nocter run examples/async.nct
```

[custom-format.nct](custom-format.nct) defines `Format` for a project-owned `Point` and uses that
ordinary static conformance from string interpolation.

```sh
nocter check examples/custom-format.nct
nocter run examples/custom-format.nct
```

[equality.nct](equality.nct) defines instance-owned equality for a project type, compares an
indexed `Vec` element, and exercises readonly plus readwrite Vec indexing.

```sh
nocter check examples/equality.nct
nocter run examples/equality.nct
```

[indexing.nct](indexing.nct) defines readonly and readwrite index operators for a project-owned
collection, assigns through the resulting place, and satisfies a generic structural index
requirement.

```sh
nocter check examples/indexing.nct
nocter run examples/indexing.nct
```

[recovery.nct](recovery.nct) turns a fallible result into a local fallback value and continues the
surrounding function. The caught `error` remains available while the fallback is computed.

```sh
nocter check examples/recovery.nct
nocter run examples/recovery.nct
```

[mutable-iteration.nct](mutable-iteration.nct) updates aggregate elements through `&+Vec<T>`, then
observes the same values through readonly and consuming expansion.

```sh
nocter check examples/mutable-iteration.nct
nocter run examples/mutable-iteration.nct
```

[ordering.nct](ordering.nct) defines strict ordering for a project type, uses the same structural
requirement from generic code, and compares `String` plus `Vec` values through standard source
declarations and readonly coercions.

```sh
nocter check examples/ordering.nct
nocter run examples/ordering.nct
```

[tuples.nct](tuples.nct) returns an owned structural tuple, mutates a decimal projection,
projects an rvalue, destructures named and discarded positions, and lets ordinary reverse-order
destruction clean up owned elements.

```sh
nocter check examples/tuples.nct
nocter run examples/tuples.nct
```

[elapsed.nct](elapsed.nct) measures one monotonic interval and blocks for a normalized `Duration`.
It demonstrates that `std/time` keeps target counter values and wait details behind its public
`Instant`, `Duration`, and `time.sleep_blocking` contracts.

```sh
nocter check examples/elapsed.nct
nocter run examples/elapsed.nct
```

[floating-point.nct](floating-point.nct) parses and calculates a binary64 value, formats it through
interpolation, converts it to an exact JSON `Number`, and generates compact JSON through the shared
shortest-decimal authority.

```sh
nocter check examples/floating-point.nct
nocter run examples/floating-point.nct
```

[unicode-text.nct](unicode-text.nct) trims Unicode whitespace, applies full default casing including
contextual and expanding mappings, queries a scalar property, and removes owned UTF-8 suffixes only
at scalar boundaries.

```sh
nocter check examples/unicode-text.nct
nocter run examples/unicode-text.nct
```

[network-address.nct](network-address.nct) parses and canonically formats one numeric IPv6 socket
address in explicit single-file mode. It performs no DNS lookup or network I/O.

```sh
nocter check examples/network-address.nct
nocter run examples/network-address.nct
```

[url-inspect.nct](url-inspect.nct) parses one absolute HTTP URL and prints its canonical URL,
authority, and request-target projections. It demonstrates that consumers use one retained URL
value rather than reparsing source text, and performs no network I/O.

```sh
nocter check examples/url-inspect.nct
nocter run examples/url-inspect.nct
```

## Package Examples

[archive-inspect/index.nct](archive-inspect/index.nct) validates and inspects a gzip-compressed
POSIX ustar archive without external compression or archive tools. One explicit policy bounds
compressed input, uncompressed output, entry count, and normalized path length. The application
rejects traversal, duplicate paths, unsupported entry kinds, and damaged gzip or tar framing. It
retains only bounded decode storage and entry metadata, and prints nothing until the complete tar
end marker, gzip trailer, and transport EOF have all been validated.

```sh
cd examples/archive-inspect
nocter check
nocter test
nocter run -- archive.tar.gz
```

[binary-record/index.nct](binary-record/index.nct) writes two portable records as one append-only
log, reads the exact bytes back asynchronously, and decodes two-byte input fragments without
host-endian assumptions. Each 23-byte record contains a magic number, version, canonical ULEB128
payload length, fixed-schema payload, and big-endian CRC-32. [wire.nct](binary-record/wire.nct)
owns emission, [parsing.nct](binary-record/parsing.nct) owns damaged-tail classification, and
[streams.nct](binary-record/streams.nct) adapts one fragmented source to both `BlockingReader` and
`Reader`; neither transport repeats framing or checksum policy. Declared tests exercise one-byte
blocking fragments, every truncated prefix, malformed lengths, checksum damage, and trailing
corruption. The executable reports success through its exit status and leaves the deterministic
46-byte wire image at the requested path.

```sh
cd examples/binary-record
nocter check
nocter test
nocter run -- record.bin
```

[async-file-report/index.nct](async-file-report/index.nct) recursively discovers regular files,
counts their bytes through bounded asynchronous chunks, and writes a deterministic summary through
a buffered file endpoint. The complete operation has a finite timeout and writes to a temporary
path first; only an explicitly closed successful report is renamed into place. Discovery or stream
failure removes the temporary path and never publishes a partial final report.

```sh
cd examples/async-file-report
nocter check
nocter run -- sample report.txt
```

[async-udp/index.nct](async-udp/index.nct) exchanges explicit-address and connected UDP datagrams
over IPv4 loopback with finite asynchronous deadlines. It checks the sender address, exact message
boundary, byte content, and truncation state without exposing nonblocking descriptor configuration
or depending on external network access.

```sh
cd examples/async-udp
nocter check
nocter run
```

[async-http/index.nct](async-http/index.nct) runs a complete HTTP/1.1 exchange over a kernel-selected
IPv4 loopback port. A structured join drives the async `Client` and `Server` together. The server
accepts one typed connection, streams one bounded `IncomingRequest` body through `Reader`, then
consumes that request to obtain its unique `Responder`. A chunked `ResponseWriter` emits two body
fragments under transport backpressure. The client applies an executor-safe per-read timeout
through the same generic reader contract and validates the response as UTF-8 without external DNS
or Internet availability.

```sh
cd examples/async-http
nocter check
nocter run
```

[http-service/index.nct](http-service/index.nct) runs seven loopback connections through a
service-owned, two-slot `TaskGroup` and a deterministic `Router` of heterogeneous `Handler` values.
The application handles a decoded path parameter and a query pair lent directly from the retained
request target and decoder scratch; it compares that pair before the next decoder advance without
creating an owned query copy. It also exercises explicit 404 and 405 policy, an intentional handler
failure, malformed framing, and an idle request timeout. One connection
pipelines two requests without concurrent execution: the first streams a 32 KiB request through a
1 KiB application buffer and returns a 32 KiB chunked response under transport backpressure; only
then does the handler decode the retained second request and force terminal response policy. The
accept loop observes one completed handler before exceeding its selected capacity. After its final
acceptance it closes listener admission, then moves the remaining handler group into one drain
operation under a fixed shutdown deadline. There is no complete-body allocation in the streaming
handler, detached task, hidden server registry, concurrent pipeline executor, or second HTTP
parser. Every accept, request read, response write, client operation, and complete shutdown drain
has a finite deadline.

```sh
cd examples/http-service
nocter check
nocter run
```

[async-loopback/index.nct](async-loopback/index.nct) starts TCP connection and listener acceptance
concurrently, exchanges newline-delimited `ping` and `pong` through generic `BufReader<TcpStream>`
and `BufWriter<TcpStream>` state, and bounds the complete exchange with structured timeout
composition. The example uses only IPv4 loopback and a kernel-selected port.

```sh
cd examples/async-loopback
nocter check
nocter run
```

[http-get/index.nct](http-get/index.nct) is a synchronous one-request HTTP client. It parses one
command-line URL, resolves its host through the operating system, applies a finite connection and
per-operation stream timeout, reads the bounded decoded response body, and writes exact body bytes
after the response status. `http://` uses a plain connection; `https://` uses authenticated TLS 1.2
or newer, the operating-system trust store, hostname verification, and HTTP/1.1 ALPN through the
same request and response implementation.

```sh
cd examples/http-get
nocter check
nocter build
./http-get http://localhost:8000/example
```

[network-loopback/index.nct](network-loopback/index.nct) performs deterministic TCP and UDP
round trips over IPv4 loopback, then observes a finite monotonic receive timeout through the stable
`std.net.timed_out` error. It uses no external host, DNS lookup, or predicted free port.

```sh
cd examples/network-loopback
nocter check
nocter run
```

[wall-clock/index.nct](wall-clock/index.nct) observes the system wall clock and reports a file's
last-content-modification instant. Both values use the same `SystemTime` domain and canonical UTC
RFC 3339 generator; target timestamp layouts remain behind `std/fs` and `std/time`.

```sh
cd examples/wall-clock
nocter check
nocter run -- ../../README.md
```

[subprocess-configured/index.nct](subprocess-configured/index.nct) configures an exact child
environment and working directory, replaces finite standard input, captures both output streams,
and observes the terminal status through the explicit blocking closed operation. Repeated `env`
calls demonstrate last-write-wins replacement; `clear_env` and `remove_env` make the final child
environment exact.

```sh
cd examples/subprocess-configured
nocter check
nocter run
```

[subprocess-output/index.nct](subprocess-output/index.nct) captures complete standard output and
standard error from the repository-owned `helper.sh`, validates both byte streams as UTF-8 text,
and inspects the helper's typed nonzero exit status. Capture preserves arbitrary bytes; the example
chooses text conversion explicitly because it knows the helper's output contract.

```sh
cd examples/subprocess-output
nocter check
nocter run
```

[subprocess-pipeline/index.nct](subprocess-pipeline/index.nct) connects one producer's stdout to a
consumer's stdin through generic executor-safe `io.copy`. Producer diagnostics, consumer output,
consumer diagnostics, and both exact-child observations progress as structured computations under
one finite timeout. Each producer stream exceeds ordinary pipe capacity, so sequential transfer
would deadlock; cancellation instead destroys the complete owned operation graph.

```sh
cd examples/subprocess-pipeline
nocter check
nocter run
```

[subprocess-status/index.nct](subprocess-status/index.nct) launches the repository-owned
`helper.sh` by the exact relative path `./helper.sh`, passes one
whitespace-bearing argument, waits synchronously, and reports the resulting typed nonzero exit
status. The example does not invoke a shell API or perform `PATH` lookup.

```sh
cd examples/subprocess-status
nocter check
nocter run
```

[text-banner/index.nct](text-banner/index.nct) turns one command-line argument into a compact text
report. Its [banner.nct](text-banner/banner.nct) implementation composes borrowed ASCII trimming,
owned replacement and repetition, interpolation, integer formatting, and symmetric line output.
The missing-argument path writes its usage message to standard error.

```sh
cd examples/text-banner
nocter check
nocter run -- "  alpha beta  "
```

[stdin-prefix/index.nct](stdin-prefix/index.nct) prefixes every logical standard-input line with
one exact command-line argument. Its [prefix.nct](stdin-prefix/prefix.nct) implementation combines
the borrowed `io.stdin()` stream with explicit `BlockingBufReader` state and ordinary
`BlockingWriter` output;
there is no global line buffer or input-specific compiler path.

```sh
cd examples/stdin-prefix
nocter check
nocter run -- '> ' < sample.txt
```

[json-normalize/index.nct](json-normalize/index.nct) reads one UTF-8 JSON file, validates and owns
its complete value, then writes the shared compact spelling directly to standard output. Its
[normalize.nct](json-normalize/normalize.nct) implementation composes process arguments, paths,
filesystem input, JSON parsing, and the public `BlockingWriter` generator without a JSON-specific file or
operating-system API.

```sh
cd examples/json-normalize
nocter check
nocter build
./json-normalize ../../example.json
```

[line-frequency/index.nct](line-frequency/index.nct) counts logical input lines with `Map`,
tracks distinct lines with `Set`, and reports the frequency of a requested line. Its
[frequency.nct](line-frequency/frequency.nct) implementation demonstrates the prelude collection
surface, keyed mapping operations, indexing mutation, ownership transfer, process arguments,
fallible file input, and a package root containing only its tiny executable adapter.

```sh
cd examples/line-frequency
nocter check
nocter build
./line-frequency ../../README.md Nocter
```

[file-summary/index.nct](file-summary/index.nct) reads a UTF-8 path from the first command-line
argument and reports the number of newline bytes. Its private
[summary.nct](file-summary/summary.nct) helper demonstrates a direct source-visibility edge without
creating another namespace. The package also demonstrates process arguments, owned paths,
`std/fs` whole-file UTF-8 input, borrowed byte views, and numeric formatting.

```sh
cd examples/file-summary
nocter check
nocter build
./file-summary ../../README.md
```

[text-report/index.nct](text-report/index.nct) is a larger command-line application. It reads a
UTF-8 file, counts logical lines and lines containing a requested string, and renders a small
report. Its [report/index.nct](text-report/report/index.nct) child module is a short public API
contract; [analysis.nct](text-report/report/analysis.nct) contains the reciprocal private
representation and implementation. The example combines directory-module imports, direct source
visibility, opaque public types, borrowed line iteration, string search, owned string construction,
numeric formatting, process arguments, and fallible file I/O.

```sh
cd examples/text-report
nocter check
nocter build
./text-report ../../README.md Nocter
```

[text-search/index.nct](text-search/index.nct) is a recursive streaming command-line application.
It classifies directory entries without following symbolic links, orders relative paths
deterministically, reuses one line destination, reports recoverable failures on standard error,
and gives match, no-match, and error distinct exit statuses. Its exact CLI contract and bounded
storage behavior are recorded in the package [README](text-search/README.md).

```sh
cd examples/text-search
nocter check
nocter build
./text-search Nocter ../../spec
```

## Contract

- Examples are complete user programs, not compiler fixtures.
- Single-file examples must pass explicit file-mode `nocter check`; package examples must pass
  package-mode `nocter check` with the distributed standard library.
- Runnable examples must remain buildable and executable on the implemented target.
- Executable fixtures are repository-owned files and must retain their executable mode.
- Source uses the canonical formatter style.
- Examples demonstrate behavior defined by the [language specification](../spec/README.md); they do
  not define additional language rules.
- Deliberately invalid compiler inputs belong under
  `development/compiler/tests/fixtures/source_corpus/`, not in this directory.
