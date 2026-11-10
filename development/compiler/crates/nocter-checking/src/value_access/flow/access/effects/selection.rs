//! One structural access path, with evaluated value facts and no recipe-equality assumption.
use super::super::{
    IndexFact,
    state::{Key, Value},
};
use super::{Context, Failure, Result};
use crate::value_access::IndexAction;
use nocter_model::{AccessExpressionId, FieldPath, ValueAccessMode as Mode, ValueProjection};
impl Context<'_, '_> {
    pub(super) fn index_failure(&self, id: AccessExpressionId) -> Result<()> {
        let expression = self
            .typed
            .body()
            .expressions()
            .get(id)
            .ok_or(Failure::Integrity)?;
        let crate::value_access::ExpressionKind::Index {
            base,
            index,
            bounds: nocter_model::IndexBoundsCheck::ProvenTrap,
            ..
        } = expression.kind
        else {
            return Err(Failure::Integrity);
        };
        if expression.ty.may_complete() {
            return Err(Failure::Integrity);
        }
        if let Some((region, _)) = self.base_value(self.world.value(base)?)? {
            self.world.check_lifetime(&region, self.at)?;
        }
        self.read(self.world.value(index)?)
    }
    pub(super) fn index(
        &mut self,
        id: AccessExpressionId,
        base: AccessExpressionId,
        index: AccessExpressionId,
        action: IndexAction,
    ) -> Result<Value> {
        if action == IndexAction::CopyPayload {
            self.read(self.world.value(base)?)?;
            return Ok(Value::without_referent(Mode::Own));
        }
        let Some((mut region, actor)) = self.base_value(self.world.value(base)?)? else {
            return Err(Failure::Integrity);
        };
        let index = self
            .typed
            .body()
            .expressions()
            .get(index)
            .ok_or(Failure::Integrity)?
            .normal_integer()
            .and_then(|value| u64::try_from(value).ok())
            .map_or(IndexFact::Unknown, IndexFact::Known);
        region.fields = region.fields.project(ValueProjection::Array(index));
        self.retain_place(Key::Expression(id), Some((region, actor)))
    }
    pub(super) fn select(
        &mut self,
        id: AccessExpressionId,
        source: AccessExpressionId,
        fields: &FieldPath,
    ) -> Result<Value> {
        let Some((mut region, actor)) = self.base_value(self.world.value(source)?)? else {
            return Err(Failure::Integrity);
        };
        region.project(fields);
        self.retain_place(Key::Expression(id), Some((region, actor)))
    }
}
