//! One operation vocabulary for checked and concretely instantiated payload obligations.
use super::{ArrayFrontier, Holder, IndexBacking, Missing, Payload, Place, RetainedIndex};
use nocter_model::AccessExpressionId;
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Step<P = Payload> {
    Capture {
        source: Place,
        destination: Holder,
        environment: P,
        payload: P,
    },
    /// Reclassify one selected named witness without constructing or duplicating its value.
    AdaptFunction {
        source: Place,
        destination: Holder,
        from: P,
        payload: P,
    },
    /// Transfer listed component obligations into one structural value; no second value is constructed.
    Assemble {
        destination: Holder,
        payload: P,
        fields: Box<[(nocter_model::ValueField, Place)]>,
    },
    Construct {
        destination: Holder,
        payload: P,
    },
    /// Receive the callee's existing result obligation, without acquiring another resource.
    Receive {
        call: AccessExpressionId,
        destination: Holder,
        payload: P,
    },
    Move {
        source: Place,
        destination: Place,
        payload: P,
    },
    MoveArrayElement {
        array: Place,
        index: RetainedIndex,
        destination: Holder,
        payload: P,
    },
    ConsolidateArrayPrefix {
        array: Place,
        payload: P,
        frontier: ArrayFrontier,
    },
    Dispose {
        source: Place,
        payload: P,
    },
    /// Dispose remaining array elements/components and end their container. Missing coverage
    /// preserves point/prefix roles; cleanup consumes their existing live scalar backing.
    DisposeArrayRemainder {
        source: Place,
        payload: P,
        missing: Box<[Missing]>,
    },
    /// Dispose initialized elements in [start, end) in reverse index order, retaining the array
    /// container. Payload is the array's obligation, not a fabricated subarray type.
    DisposeArrayElements {
        source: Place,
        payload: P,
        range: nocter_model::ArrayRange<IndexBacking>,
    },
    /// End a partial structural container's slot after cleaning its surviving regions. This does
    /// not destroy its payload again or prescribe physical allocation/deallocation.
    ReleaseContainer {
        source: Place,
        payload: P,
    },
    Invoke {
        source: Place,
        call: AccessExpressionId,
        index: usize,
        payload: P,
    },
    /// Complete replacement through a scoped update alias. Its existing referent is initialized;
    /// the enclosing owner's completeness does not change and no alias becomes an owner.
    ReplaceReferent {
        destination: Place,
        source: Place,
        payload: P,
    },
}
impl<P> Step<P> {
    /// Project payload obligations without selecting another operation or changing place identity.
    /// # Errors
    /// Returns the supplied projection's error without publishing a partly transformed step.
    // One exhaustive structural transform keeps every operation and its payload fields together.
    #[allow(clippy::too_many_lines)]
    pub fn try_map_payloads<E, Q>(
        &self,
        mut map: impl FnMut(&P) -> std::result::Result<Q, E>,
    ) -> std::result::Result<Step<Q>, E> {
        Ok(match self {
            Self::Capture {
                source,
                destination,
                environment,
                payload,
            } => Step::Capture {
                source: source.clone(),
                destination: *destination,
                environment: map(environment)?,
                payload: map(payload)?,
            },
            Self::AdaptFunction {
                source,
                destination,
                from,
                payload,
            } => Step::AdaptFunction {
                source: source.clone(),
                destination: *destination,
                from: map(from)?,
                payload: map(payload)?,
            },
            Self::Assemble {
                destination,
                payload,
                fields,
            } => Step::Assemble {
                destination: *destination,
                payload: map(payload)?,
                fields: fields.clone(),
            },
            Self::Construct {
                destination,
                payload,
            } => Step::Construct {
                destination: *destination,
                payload: map(payload)?,
            },
            Self::Receive {
                call,
                destination,
                payload,
            } => Step::Receive {
                call: *call,
                destination: *destination,
                payload: map(payload)?,
            },
            Self::Move {
                source,
                destination,
                payload,
            } => Step::Move {
                source: source.clone(),
                destination: destination.clone(),
                payload: map(payload)?,
            },
            Self::MoveArrayElement {
                array,
                index,
                destination,
                payload,
            } => Step::MoveArrayElement {
                array: array.clone(),
                index: *index,
                destination: *destination,
                payload: map(payload)?,
            },
            Self::ConsolidateArrayPrefix {
                array,
                payload,
                frontier,
            } => Step::ConsolidateArrayPrefix {
                array: array.clone(),
                payload: map(payload)?,
                frontier: *frontier,
            },
            Self::Dispose { source, payload } => Step::Dispose {
                source: source.clone(),
                payload: map(payload)?,
            },
            Self::DisposeArrayRemainder {
                source,
                payload,
                missing,
            } => Step::DisposeArrayRemainder {
                source: source.clone(),
                payload: map(payload)?,
                missing: missing.clone(),
            },
            Self::DisposeArrayElements {
                source,
                payload,
                range,
            } => Step::DisposeArrayElements {
                source: source.clone(),
                payload: map(payload)?,
                range: *range,
            },
            Self::ReleaseContainer { source, payload } => Step::ReleaseContainer {
                source: source.clone(),
                payload: map(payload)?,
            },
            Self::Invoke {
                source,
                call,
                index,
                payload,
            } => Step::Invoke {
                source: source.clone(),
                call: *call,
                index: *index,
                payload: map(payload)?,
            },
            Self::ReplaceReferent {
                destination,
                source,
                payload,
            } => Step::ReplaceReferent {
                destination: destination.clone(),
                source: source.clone(),
                payload: map(payload)?,
            },
        })
    }
}
