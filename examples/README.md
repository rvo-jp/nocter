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
IPv4 loopback port. A structured join drives the async client and a small local peer together. The
client uses validated text request conveniences and a generic `Reader` algorithm that applies the
same executor-safe per-read timeout to both a `TcpStream` and an HTTP `Response`. It collects and
validates the response as UTF-8 without external DNS or Internet availability.

```sh
cd examples/async-http
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
and observes the terminal status as one synchronous operation. Repeated `env` calls demonstrate
last-write-wins replacement; `clear_env` and `remove_env` make the final child environment exact.

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
