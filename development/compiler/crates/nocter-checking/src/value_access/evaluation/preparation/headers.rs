//! Source-specific header construction behind the one body/value dependency coordinator.
use super::{SourceFailure, declarations};
use nocter_declarations::value_access::DeclarationId;
use nocter_model::{AccessCodeId, FrozenData};

pub(super) trait SourceHeaders<'source> {
    fn advance_with_values<'value>(
        &mut self,
        id: DeclarationId,
        values: &mut dyn FnMut(AccessCodeId) -> Option<&'value FrozenData>,
    ) -> Result<declarations::HeaderProgress, SourceFailure>;
    fn current(&self) -> Result<declarations::Context<'source>, SourceFailure>;
    fn reject_cycle(&self, path: &[DeclarationId]) -> SourceFailure;
}
pub(super) enum Headers<'a, 'source> {
    Closed,
    Publishing(&'a mut dyn SourceHeaders<'source>),
}
impl<'source> SourceHeaders<'source> for declarations::Preparation<'source> {
    fn advance_with_values<'value>(
        &mut self,
        id: DeclarationId,
        values: &mut dyn FnMut(AccessCodeId) -> Option<&'value FrozenData>,
    ) -> Result<declarations::HeaderProgress, SourceFailure> {
        declarations::Preparation::advance_with_values(self, id, values).map_err(Into::into)
    }
    fn current(&self) -> Result<declarations::Context<'source>, SourceFailure> {
        Ok(declarations::Preparation::current(self))
    }
    fn reject_cycle(&self, path: &[DeclarationId]) -> SourceFailure {
        declarations::Preparation::reject_cycle(self, path).into()
    }
}
pub(super) struct ModuleHeaders<'a, 'source, 'syntax> {
    pub owner: &'a mut declarations::ScopedModuleSources<'syntax>,
    pub binding: &'source declarations::ModuleSourceBinding<'syntax>,
}
impl<'source> SourceHeaders<'source> for ModuleHeaders<'_, 'source, '_> {
    fn advance_with_values<'value>(
        &mut self,
        id: DeclarationId,
        values: &mut dyn FnMut(AccessCodeId) -> Option<&'value FrozenData>,
    ) -> Result<declarations::HeaderProgress, SourceFailure> {
        self.owner
            .advance_header_with_values(id, values)
            .map_err(Into::into)
    }
    fn current(&self) -> Result<declarations::Context<'source>, SourceFailure> {
        self.binding.current(self.owner).map_err(Into::into)
    }
    fn reject_cycle(&self, path: &[DeclarationId]) -> SourceFailure {
        self.owner.reject_cycle(path).into()
    }
}
