//! Consume resolved source locators once; checked semantic role facts belong to `PrimitiveBindings`.
use super::{
    ModuleSourceBinding, ModuleSourcesFailure, ParsedHeader, ScopedModuleSources, integrity, syntax,
};
use crate::access_declarations::PrimitiveBindings;
use nocter_declarations::value_access::DeclarationId;
use nocter_model::AccessCodeId;
use std::collections::BTreeMap;

pub(super) fn select(
    binding: &ModuleSourceBinding<'_>,
    resolved: Vec<crate::toolchain::ResolvedPrimitiveRole>,
) -> Result<PrimitiveBindings, ModuleSourcesFailure> {
    let mut selected_codes = BTreeMap::new();
    for role in resolved {
        let Some(DeclarationId::Code(code)) = binding.declarations[role.declaration().index()]
        else {
            return Err(integrity());
        };
        if selected_codes.insert(code, role.role()).is_some() {
            return Err(integrity());
        }
    }
    let mut bindings = PrimitiveBindings::default();
    for (raw, header) in &binding.catalog.parsed {
        if let ParsedHeader::Code(header) = header {
            let Some(DeclarationId::Code(code)) = binding.declarations[raw.index()] else {
                return Err(integrity());
            };
            let physical = binding.paired.inventory.declarations()[raw.index()];
            bindings
                .select_role(
                    syntax(&binding.paired, physical.source())?,
                    selected_codes.get(&code).copied(),
                    code,
                    header,
                )
                .map_err(ModuleSourcesFailure::Access)?;
        }
    }
    Ok(bindings)
}
impl ScopedModuleSources<'_> {
    pub(super) fn validate_primitive(
        &mut self,
        code: AccessCodeId,
    ) -> Result<(), ModuleSourcesFailure> {
        if !self.primitives.needs_validation(code) {
            return Ok(());
        }
        let scope = self.binding.code_scope(code).map_err(|_| integrity())?;
        let source = self
            .binding
            .catalog
            .sources
            .codes
            .get(&code)
            .ok_or_else(integrity)?;
        self.primitives
            .validate(scope.syntax, &self.headers.snapshot(), code, source)
            .map_err(ModuleSourcesFailure::Access)
    }
}
