//! Source permission over the existing representation/field identities, never another schema.
use super::ModuleSourceBinding;
use crate::SurfaceSourceId;
use nocter_declarations::value_access::DeclarationId;
use nocter_model::{AccessFieldId, NominalTypeId};
use nocter_syntax::BoundSyntax;

impl ModuleSourceBinding<'_> {
    pub(in crate::access_declarations) fn record_construction_allowed(
        &self,
        syntax: BoundSyntax<'_>,
        record: NominalTypeId,
    ) -> bool {
        let Some(source) = self.source_for_view(syntax) else {
            return false;
        };
        if self.complete_record_visible_from(source, record) {
            return true;
        }
        let inventory = &self.paired.inventory;
        let caller = inventory.sources()[source.index()].module();
        self.paired
            .contracts
            .structural_grants()
            .any(|(raw, grant)| {
                self.declarations[raw.index()] == Some(DeclarationId::Record(record))
                    && inventory
                        .audience(grant)
                        .is_some_and(|audience| audience.contains(caller))
            })
    }
    pub(in crate::access_declarations) fn field_visible(
        &self,
        syntax: BoundSyntax<'_>,
        record: NominalTypeId,
        field: AccessFieldId,
    ) -> bool {
        let Some(source) = self.source_for_view(syntax) else {
            return false;
        };
        if self.complete_record_visible_from(source, record) {
            return true;
        }
        let inventory = &self.paired.inventory;
        let caller = inventory.sources()[source.index()].module();
        inventory.declaration_entries().any(|(raw, declaration)| {
            self.fields[raw.index()] == Some(field)
                && declaration
                    .owner()
                    .and_then(|owner| self.declarations[owner.index()])
                    == Some(DeclarationId::Record(record))
                && inventory
                    .audience(raw)
                    .is_some_and(|audience| audience.contains(caller))
        })
    }
    fn source_for_view(&self, syntax: BoundSyntax<'_>) -> Option<SurfaceSourceId> {
        self.paired
            .inventory
            .source_entries()
            .find(|(_, source)| source.syntax().root_id() == syntax.tree().root_id())
            .map(|(id, _)| id)
    }
    fn complete_record_visible_from(&self, source: SurfaceSourceId, record: NominalTypeId) -> bool {
        let Some(definition) = self.catalog.sources.records.get(&record) else {
            return false;
        };
        let Some(target) = self.physical_source(definition.name.source()) else {
            return false;
        };
        super::super::module_sources::visible_sources(&self.paired, source)
            .any(|visible| visible == target)
    }
}
