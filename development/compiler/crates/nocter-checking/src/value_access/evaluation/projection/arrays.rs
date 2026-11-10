//! Array execution consumes established schema/address operations. The closed VM checks its
//! actual numeric indices even when source checking proves a native check unnecessary.
use super::{Builder, INVALID, Result, plan};
use crate::value_access::IndexAction;
use nocter_model::AccessExpressionId;
impl Builder<'_, '_> {
    pub(super) fn repeat_operation(
        &mut self,
        id: AccessExpressionId,
        seed: AccessExpressionId,
        destination: usize,
    ) -> Result<plan::Operation> {
        let Some(nocter_declarations::value_access::PayloadSchema::Array(array)) =
            self.program.data_schema(self.data(id)?)
        else {
            return Err(INVALID);
        };
        Ok(if array.length() > 1 {
            plan::Operation::Repeat {
                destination,
                source: self.value(seed)?,
            }
        } else {
            plan::Operation::None
        })
    }
    pub(super) fn index_operation(
        &mut self,
        destination: usize,
        base: AccessExpressionId,
        index: AccessExpressionId,
        action: IndexAction,
    ) -> Result<plan::Operation> {
        let base = self.value(base)?;
        let index = self.value(index)?;
        Ok(match action {
            IndexAction::Access => plan::Operation::Index {
                destination,
                base,
                index,
            },
            IndexAction::CopyPayload => plan::Operation::IndexCopy {
                destination,
                base,
                index,
            },
        })
    }
}
