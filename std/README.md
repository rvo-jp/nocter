# Standard Library

This directory is the sole authority for the portable public modules distributed with Nocter.
Every module `index.nct` owns its compiler-checked public declarations and declaration doc
comments. The catalog below assigns each longer observable subject to one behavior guide. A guide
may span tightly coupled modules when separating their cross-operation invariants would create
duplicate authorities. Declarations answer what can be called; guides explain observable behavior,
failure, complexity, and invariants without repeating signatures.
The package identity itself is defined by the checked [root declaration](index.nct).

Language constructs and target primitives are referenced from their owning specification chapters
rather than redefined here. Generated contract pages show the exact checked `index.nct` source:
only declarations with public language visibility are public API, while visible `see` edges and
restricted declarations remain source-level module assembly and package contracts. Independent
package-only modules live under `std/internal` and are excluded from generated navigation. The
guides below cover modules whose observable behavior needs more explanation than declaration
comments.

## Behavior Guide Authority

| Observable subject | Sole guide | Checked contracts |
| --- | --- | --- |
| borrowed UTF-8 text | [Borrowed Text](str/README.md) | `std/str` |
| owned UTF-8 text | [Owned Strings](string/README.md) | `std/string` |
| growable sequences | [Vectors](vec/README.md) | `std/vec` |
| inline fixed-capacity sequences and byte staging | [Fixed-Capacity Collections](fixed/README.md) | `std/fixed` |
| borrowed sequences | [Slices](slice/README.md) | `std/slice` |
| borrowed parsing windows | [Borrowed Scanning](scan/README.md) | `std/scan` |
| fixed-width binary scalar encoding | [Binary Scalar Codecs](bytes/README.md) | `std/bytes` |
| canonical hexadecimal bytes | [Hexadecimal Bytes](hex/README.md) | `std/hex` |
| standard and URL-safe Base64 bytes | [Base64 Bytes](base64/README.md) | `std/base64` |
| cryptographic content digests | [Cryptographic Digests](digest/README.md) | `std/digest` |
| accidental-corruption checksums | [CRC-32 Checksums](checksum/README.md) | `std/checksum` |
| streaming compressed representations | [Streaming Compression](compress/README.md) | `std/compress` |
| streaming archive entries and safe relative paths | [Streaming Tar Archives](archive/README.md) | `std/archive` |
| iteration and collection | [Iteration](iter/README.md) | `std/iter`, `std/iter/collect` |
| value formatting | [Formatting](fmt/README.md) | `std/fmt` |
| byte streams and buffering | [I/O](io/README.md) | `std/io`, `std/io/buffer` |
| paths and filesystem operations | [Filesystem](fs/README.md) | `std/path`, `std/fs` |
| durable local byte storage | [Durable Local Byte Storage](store/README.md) | `std/store` |
| source-neutral configuration, publication, and input adapters | [Source-Neutral Configuration](config/README.md) | `std/config`, `std/config/arguments`, `std/config/environment`, `std/config/json` |
| numeric values and text conversion | [Numeric Values](num/README.md) | `std/num` |
| total ordering and comparison results | [Total Ordering](order/README.md) | `std/order` |
| allocation and storage failure | [Allocation and Failure](mem/README.md) | `std/mem` |
| recoverable failure payloads | [Recoverable Errors](error/README.md) | `std/error` |
| pointers and addresses | [Pointer and Address Conversion](ptr/README.md) | `std/ptr` |
| hashing and unordered collections | [Associative Collections](map/README.md) | `std/hash`, `std/map`, `std/set` |
| cryptographically secure random bytes and scalars | [Cryptographic Randomness](random/README.md) | `std/random` |
| canonical UUID values and version-4 generation | [Universally Unique Identifiers](uuid/README.md) | `std/uuid` |
| opaque session identifiers | [Opaque Session Identifiers](session/README.md) | `std/session` |
| JSON values, parsing, and generation | [JSON Values and Text](json/README.md) | `std/json` |
| structured operational events and redaction | [Structured Operational Logging](log/README.md) | `std/log` |
| durations, monotonic time, wall-clock time, and UTC calendar values | [Time](time/README.md) | `std/time` |
| structured asynchronous computation composition | [Structured Asynchronous Tasks](task/README.md) | `std/task` |
| cooperative shared ownership and synchronization | [Cooperative Synchronization](sync/README.md) | `std/sync` |
| long-running task ownership, reload, and graceful shutdown | [Service Lifecycle](service/README.md) | `std/service` |
| synchronous processes and process context | [Synchronous Processes](process/README.md) | `std/process` |
| structured command-line parsing and help | [Command-Line Applications](cli/README.md) | `std/cli` |
| numeric network addresses and socket I/O | [Network I/O](net/README.md) | `std/net` |
| authenticated TLS client streams | [Authenticated TLS](tls/README.md) | `std/tls` |
| absolute HTTP-family URLs and request targets | [URL](url/README.md) | `std/url` |
| HTTP/1.1 client, server, message values, and framing | [HTTP/1.1](http/README.md) | `std/http` |
| Unicode scalars and text transforms | [Unicode Text and Scalars](char/README.md) | `std/char`, Unicode operations on `std/str` and `std/string` |
| native assertions | [Native Assertions](testing/README.md) | `std/testing` |
