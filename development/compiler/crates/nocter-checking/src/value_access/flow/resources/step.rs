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
    /// The retired destination and supplied source have separate concrete obligations.
    ReplaceReferent {
        destination: Place,
        source: Place,
        retired: P,
        payload: P,
    },
}
/// The value occurrence supplying a step's payload, without an access or initialization grant.
/// A destination is the new value defined by the step; a source retains the original selected place.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PayloadSite<'a> {
    Source(&'a Place),
    Destination(Holder),
}

impl<P> Step<P> {
    /// Project payload obligations without selecting another operation or changing place identity.
    /// # Errors
    /// Returns the supplied projection's error without publishing a partly transformed step.
    pub fn try_map_payloads<E, Q>(
        &self,
        mut map: impl FnMut(&P) -> std::result::Result<Q, E>,
    ) -> std::result::Result<Step<Q>, E> {
        self.try_map_payloads_at(|_, payload| map(payload))
    }
    /// Project each obligation together with its exact source or newly defined destination.
    /// The caller retains action/step order; this does not select a state boundary or access proof.
    /// # Errors
    /// Returns the projection's error without publishing a partly transformed step.
    // One exhaustive structural transform keeps operation identities and both APIs consistent.
    #[allow(clippy::too_many_lines)]
    pub fn try_map_payloads_at<E, Q>(
        &self,
        mut map: impl FnMut(PayloadSite<'_>, &P) -> std::result::Result<Q, E>,
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
                environment: map(PayloadSite::Source(source), environment)?,
                payload: map(PayloadSite::Destination(*destination), payload)?,
            },
            Self::AdaptFunction {
                source,
                destination,
                from,
                payload,
            } => Step::AdaptFunction {
                source: source.clone(),
                destination: *destination,
                from: map(PayloadSite::Source(source), from)?,
                payload: map(PayloadSite::Destination(*destination), payload)?,
            },
            Self::Assemble {
                destination,
                payload,
                fields,
            } => Step::Assemble {
                destination: *destination,
                payload: map(PayloadSite::Destination(*destination), payload)?,
                fields: fields.clone(),
            },
            Self::Construct {
                destination,
                payload,
            } => Step::Construct {
                destination: *destination,
                payload: map(PayloadSite::Destination(*destination), payload)?,
            },
            Self::Receive {
                call,
                destination,
                payload,
            } => Step::Receive {
                call: *call,
                destination: *destination,
                payload: map(PayloadSite::Destination(*destination), payload)?,
            },
            Self::Move {
                source,
                destination,
                payload,
            } => Step::Move {
                source: source.clone(),
                destination: destination.clone(),
                payload: map(PayloadSite::Source(source), payload)?,
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
                payload: map(
                    PayloadSite::Source(&array.project_retained(*index)),
                    payload,
                )?,
            },
            Self::ConsolidateArrayPrefix {
                array,
                payload,
                frontier,
            } => Step::ConsolidateArrayPrefix {
                array: array.clone(),
                payload: map(PayloadSite::Source(array), payload)?,
                frontier: *frontier,
            },
            Self::Dispose { source, payload } => Step::Dispose {
                source: source.clone(),
                payload: map(PayloadSite::Source(source), payload)?,
            },
            Self::DisposeArrayRemainder {
                source,
                payload,
                missing,
            } => Step::DisposeArrayRemainder {
                source: source.clone(),
                payload: map(PayloadSite::Source(source), payload)?,
                missing: missing.clone(),
            },
            Self::DisposeArrayElements {
                source,
                payload,
                range,
            } => Step::DisposeArrayElements {
                source: source.clone(),
                payload: map(PayloadSite::Source(source), payload)?,
                range: *range,
            },
            Self::ReleaseContainer { source, payload } => Step::ReleaseContainer {
                source: source.clone(),
                payload: map(PayloadSite::Source(source), payload)?,
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
                payload: map(PayloadSite::Source(source), payload)?,
            },
            Self::ReplaceReferent {
                destination,
                source,
                retired,
                payload,
            } => Step::ReplaceReferent {
                destination: destination.clone(),
                source: source.clone(),
                retired: map(PayloadSite::Source(destination), retired)?,
                payload: map(PayloadSite::Source(source), payload)?,
            },
        })
    }
}
