/// One direction of nonblocking descriptor progress.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ReadinessDirection {
    Readable,
    Writable,
}

/// One opaque asynchronous wait interest before target ABI encoding.
///
/// Timer deadlines use the wrapping monotonic counter domain supplied to the selected reactor.
/// A deadline denotes a future distance no greater than half that domain; zero distance and the
/// upper half are eligible. Operation code remains the authority for interpreting readiness,
/// timeout, closure, and error after the computation resumes.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ReactorInterest {
    Descriptor {
        descriptor: u64,
        direction: ReadinessDirection,
    },
    Timer {
        deadline: u64,
    },
    /// Completion of one exact process owner selected before suspension.
    ///
    /// The subject is an opaque target process identity, not a source-visible PID ownership
    /// token. Operation code remains responsible for observing and classifying termination after
    /// wakeup.
    ProcessExit {
        process: u64,
    },
}
