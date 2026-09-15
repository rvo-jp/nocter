# Streaming HTTP Service Boundary

This document defines the ownership and protocol boundary for bounded streaming and sequential
persistent HTTP/1.1 services. Exact public declarations belong to
`development/std/http/index.nct`, observable HTTP behavior belongs to
`development/std/http/README.md`, and asynchronous execution semantics belong to the
[Asynchronous Computation Boundary](asynchronous-computation-design.md).

## Outcome

Client response input and server request input use one transport-independent body cursor. The
cursor owns framing progress and buffered byte position; transport drivers own only byte movement,
timeouts, and stream closure. Server typestates transfer one connection linearly from acceptance,
through request consumption and response production, to either explicit closure or safe reuse.

Streaming is not an alternative HTTP implementation. Complete-body conveniences drive the same
cursor or response writer until completion. No adapter may parse framing fields, maintain another
completion flag, or decide persistence from raw headers.

## Linear Server Typestate

The server lifecycle has one ownership path:

```text
ServerConnection
    -> IncomingRequest
    -> Responder
    -> ResponseWriter
    -> ServerConnection?  // reusable only when the completed exchange permits it
```

`ServerConnection` owns an accepted transport and any bytes retained from a prior exchange.
Reading the next head consumes it and produces `IncomingRequest`; no responder exists at the same
time. The incoming request owns the parsed head, body cursor, transport, limits, peer identity, and
already selected connection disposition.

The incoming request supplies ordinary `Reader` and `TimedReader` operations. A consuming
finalization operation drains any unread body through the same cursor and returns the unique
`Responder`. It has no "body must already be complete" precondition. Failure, timeout, or
cancellation while draining destroys the owner and closes the stream, so callers cannot receive a
responder paired with an incomplete request.

The responder may send an owned complete response or begin incremental output. Complete response
emission returns `ServerConnection?`: presence means the exact transport and retained input are
safe for one later request; absence means policy or peer input required closure. A caller may always
choose an explicit terminal operation instead. No public state permits request-body reads and
response writes to overlap on one stream.

## Canonical Body Cursor

One private body cursor owns:

- the selected `BodyDecoder`;
- the retained transport-input allocation;
- the first unread input offset; and
- one fixed-size transport-input scratch allocation.

The decoder's state is the sole completion authority. The cursor does not mirror completion in a
Boolean, infer it from empty storage, or ask a transport whether EOF means message completion. Its
advance result reports decoded output, need for more transport input, completion, or failure.

Completing one fixed-length, chunked, or bodyless message may leave unread bytes in the retained
allocation. Those bytes remain owned by the cursor and transfer to the responder and then to a
reusable connection. Close-delimited input completes only at transport EOF and therefore cannot
produce a reusable connection. No driver treats overread as unsupported pipelining: pipelined bytes
may be retained, but they are not parsed or executed until the prior response has completed.

The cursor is independent of `TcpStream`, TLS, client/server roles, relative timeouts, and response
policy. A transport driver may be replaced without changing framing state, and the decoder may be
replaced without changing transport ownership.

## Response Planning and Output

`ResponseHead` is the one validated status-and-fields value for parsed and authored response
metadata. An owned complete response combines that head with owned body bytes; it does not maintain
a second head representation.

Beginning output selects exactly one response plan before writing:

- bodyless for HEAD and statuses whose semantics prohibit a payload;
- fixed length for a known byte count; or
- chunked for an incrementally terminated body of unknown length.

The plan validates status, reserved fields, limits, body suppression, connection disposition, and
generated fields once. A response writer consumes only the frozen plan. Fixed-length output tracks
the remaining count and cannot finish early or accept excess bytes. Chunked output owns chunk
spelling and emits the terminal chunk exactly once. Both write directly through transport
backpressure with fixed protocol scratch storage; neither accumulates the complete payload.

The existing owned-body response operation derives a fixed-length plan, writes its borrowed body
through the same writer transition, and finalizes it. It is a convenience owner, not another
framing or connection-lifecycle authority.

`ResponseOutputState` belongs to the planner contract, not transport storage. Pure fixed-length and
chunked transition functions validate the next logical fragment before any byte is written. The
driver marks the owner failed before awaiting transport and publishes the planned next state only
after every wire byte succeeds. Cancellation therefore cannot leave a writable owner whose
reported progress exceeds its accepted wire prefix.

## Persistence Selection

Validated field processing records semantic connection tokens while it already owns header
interpretation. HTTP/1.1 persistence is the default unless a validated request requires closure.
Response policy may preserve that decision or force closure; it cannot make a non-reusable request
reusable.

Reusable connection publication requires all of these completed facts:

1. the request head selected reusable HTTP/1.1 semantics;
2. the request body cursor reached its terminal state;
3. the response plan selected reusable framing;
4. every response byte, including a chunk terminator when required, was accepted by the transport;
5. no timeout, cancellation, codec error, or transport error occurred.

This conjunction is evaluated by the responder/writer ownership transition. Callers never combine
flags or reconstruct the decision from header text.

## Deadlines, Cancellation, and Backpressure

Individual reads and writes delegate optional idle timeouts to `TimedReader` and `TimedWriter`.
Whole-operation deadlines remain compositions through `std/task`; HTTP does not own another clock
or readiness mechanism.

Cancelling a borrowed body read ends only that read and leaves the unique incoming-request owner at
its advanced cursor. Cancelling a consuming finalization or any consuming response transition
destroys its stream owner and cannot return a continuation. Response writes await transport
acceptance before advancing fixed-length or chunked state, so cancellation never reports unsent
bytes as committed or silently retries an observable prefix.

Transport buffers and protocol scratch buffers have fixed capacity independent of message size.
`Limits` continues to bound decoded message size and syntax-controlled storage; applications may
choose a larger validated body limit without creating a body-sized allocation in the streaming
path.

## Responsibility Matrix

| Decision | Sole authority | Consumers |
|---|---|---|
| Head syntax, framing fields, and message limits | HTTP codec | body cursor and response planner |
| Decoded-body progress and pending byte range | body cursor | client and server transport drivers |
| Transport progress, timeout, and close-once ownership | `std/net` and `std/tls` stream contracts | HTTP drivers |
| Request-to-response linear ownership | server typestate | application handlers |
| Response framing and generated protocol fields | response plan | complete and streaming response drivers |
| Connection reuse eligibility | completed responder/writer transition | server accept loop |
| Concurrent connection capacity and draining | application-owned `TaskGroup` | service orchestration |
| Public declarations | `development/std/http/index.nct` | compiler, editor, and applications |

## Rejected Shortcuts

- Do not return a responder beside a still-readable request body.
- Do not require callers to check an internal completion flag before requesting response authority.
- Do not preserve the current full-body server operation as an independent decoder path.
- Do not infer message completion from transport EOF except for close-delimited framing.
- Do not discard overread bytes merely because pipelined execution is unsupported.
- Do not expose raw `Connection`, `Content-Length`, or `Transfer-Encoding` control to bypass the
  response plan.
- Do not add a hidden accept loop, task registry, connection semaphore, or keep-alive pool.
- Do not make HTTP parsing or persistence a compiler primitive.
