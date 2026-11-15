//! Ordinary occurrence data is selected by checking, not reconstructed by a backend.
use super::{Failure, InstanceKey};
pub(in crate::value_access) use nocter_checking::value_access::ValueData as Values;
use nocter_checking::value_access::{ValueDataFailure, accepted::Unit};
use nocter_constant_evaluation::state::WorkBudget;

pub(in crate::value_access) fn values(
    unit: &Unit,
    frame: &InstanceKey,
    work: &mut WorkBudget,
) -> Result<Values, Failure> {
    unit.instantiate_value_data_map(frame, work)
        .map_err(|failure| match failure {
            ValueDataFailure::Body => Failure::InvalidAcceptedUnit,
            ValueDataFailure::Control(failure) => Failure::Control {
                code: frame.code(),
                failure,
            },
            ValueDataFailure::Value { at, failure } => Failure::Value {
                code: frame.code(),
                at,
                failure,
            },
        })
}
