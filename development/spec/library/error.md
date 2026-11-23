# Recoverable Errors

## Construction and Context

The [standard type contract](../../stdlib/error/index.nct) provides `error.new` and `context`.
Use `throw` to deliver the resulting payload as a failure.

```text
let cause = error.new("app.config.missing", "missing key")
throw cause^.context("reading config")
```

The context becomes the outer message on the failure graph. The original code and cause remain
available under the error contract.

## Value Access

An owned `error` retains its private immutable failure graph, not access into the views originally
used to construct its text. That graph has no caller-managed region requirement. Moving the handle
transfers its cleanup obligation; destroying it releases its owned graph according to the error
storage contract and invokes no user-defined destructor or callback. Reading the handle does not
copy it. Access returned from an accessor still depends on the live handle.

These concrete snapshot/storage facts do not make other move-only values independent. A future
aggregate, closure, or future can carry outside access and storage dependencies even when owned.
