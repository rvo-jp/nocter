//! Resolve every paired occurrence before loading one define-once semantic header.
use super::{ModuleSourcesFailure, ParsedHeader, ScopedModuleSources, integrity, syntax};
use crate::access_declarations::{
    CodeSource, HeaderProgress,
    definitions::{self, HeaderScope, Projection},
    projection,
    source::Sources,
};
use crate::{DeclarationContractError, SurfaceDeclarationId};
use nocter_declarations::value_access::{
    DataReference, DeclarationId, HeaderFailure, HeaderStep, Program,
};
use nocter_model::{AccessCodeId, FrozenData};
use std::collections::{BTreeMap, BTreeSet};

enum Definition {
    Code(definitions::CodeDefinition),
    Record(definitions::RecordDefinition),
    Enumeration(definitions::EnumDefinition),
    Alias(DataReference),
}
impl ScopedModuleSources<'_> {
    /// Advance one header using all of its physical contract/definition occurrences.
    /// # Errors
    /// Rejects local lookup/contract mismatches before loading the shared semantic definition.
    /// Completed array counts are inputs to this step, not evidence of checked source bodies.
    pub fn advance_header_with_values<'value>(
        &mut self,
        required: impl Into<DeclarationId>,
        mut value: impl FnMut(AccessCodeId) -> Option<&'value FrozenData>,
    ) -> Result<HeaderProgress, ModuleSourcesFailure> {
        let required = required.into();
        if !self.headers.is_defined(required).ok_or_else(integrity)? {
            let occurrences = self
                .binding
                .catalog
                .parsed
                .iter()
                .filter(|(raw, _)| self.binding.declarations[raw.index()] == Some(required));
            let program = self.program();
            let mut projected = BTreeMap::new();
            let mut missing = Vec::new();
            let mut missing_aliases = BTreeSet::new();
            for (raw, header) in occurrences {
                let physical = self.binding.paired.inventory.declarations()[raw.index()];
                let scope = HeaderScope {
                    syntax: syntax(&self.binding.paired, physical.source())?,
                    lookup: self.binding.declaration_scopes[raw.index()],
                };
                let Projection {
                    definition,
                    progress,
                } = project_occurrence(
                    &program,
                    required,
                    header,
                    scope,
                    &self.binding.catalog.sources,
                    &mut value,
                )?;
                if !progress.violations.is_empty() {
                    return Err(self.project_header_failure(
                        HeaderFailure::Rejected(progress.violations),
                        Some(*raw),
                    ));
                }
                missing.extend(progress.missing);
                missing_aliases.extend(progress.missing_aliases);
                if let Some(definition) = definition {
                    if physical.kind() == crate::SurfaceDeclarationKind::ConstructionFunction {
                        let (Definition::Code(code), ParsedHeader::Code(header)) =
                            (&definition, header)
                        else {
                            return Err(integrity());
                        };
                        self.binding.members.verify_result(
                            *raw,
                            scope.syntax,
                            header.result_syntax,
                            &code.result,
                        )?;
                    }
                    projected.insert(*raw, definition);
                }
            }
            if !missing_aliases.is_empty() {
                return Ok(HeaderProgress::RequireHeaders(
                    missing_aliases
                        .into_iter()
                        .map(DeclarationId::Alias)
                        .collect(),
                ));
            }
            if !missing.is_empty() {
                missing.sort_unstable();
                missing.dedup();
                return Ok(HeaderProgress::RequireValues(missing.into()));
            }
            self.define_agreed(required, projected)?;
        }
        let step = self
            .headers
            .advance(required)
            .map_err(|failure| self.project_header_failure(failure, None))?;
        if step == HeaderStep::Ready
            && let DeclarationId::Code(code) = required
        {
            self.validate_primitive(code)?;
        }
        Ok(match step {
            HeaderStep::Ready => HeaderProgress::Ready,
            HeaderStep::Require(headers) => HeaderProgress::RequireHeaders(headers),
        })
    }

    fn define_agreed(
        &mut self,
        required: DeclarationId,
        mut projected: BTreeMap<SurfaceDeclarationId, Definition>,
    ) -> Result<(), ModuleSourcesFailure> {
        let contract = *projected
            .keys()
            .find(|raw| self.binding.paired.contracts.representative(**raw) == **raw)
            .ok_or_else(integrity)?;
        let selected = self
            .binding
            .paired
            .contracts
            .representation(contract)
            .unwrap_or(contract);
        let definition = projected.remove(&selected).ok_or_else(integrity)?;
        for (raw, other) in &projected {
            let agrees = match (&definition, other) {
                (Definition::Code(a), Definition::Code(b)) => a == b,
                (Definition::Record(complete), Definition::Record(public)) => {
                    self.records_agree(required, *raw, complete, public)?
                }
                (
                    Definition::Enumeration(definitions::EnumDefinition {
                        copy: complete,
                        variants: Some(_),
                    }),
                    Definition::Enumeration(definitions::EnumDefinition {
                        copy: public,
                        variants: None,
                    }),
                ) => complete == public,
                _ => false,
            };
            if !agrees {
                return Err(self.mismatch(contract, if *raw == contract { selected } else { *raw }));
            }
        }
        match (required, definition) {
            (DeclarationId::Code(id), Definition::Code(definition)) => self.headers.define(
                id,
                definition.kind,
                definition.parameters,
                definition.result,
            ),
            (DeclarationId::Nominal(id), Definition::Record(definition)) => self
                .headers
                .define_record(id, definition.copy, definition.fields),
            (
                DeclarationId::Nominal(id),
                Definition::Enumeration(definitions::EnumDefinition {
                    copy,
                    variants: Some(variants),
                }),
            ) => {
                let exposed_scalar = selected == contract
                    && variants.iter().all(|variant| variant.fields.is_empty());
                self.headers
                    .define_enum(id, copy || exposed_scalar, variants)
            }
            (DeclarationId::Alias(id), Definition::Alias(target)) => {
                self.headers.define_alias(id, target)
            }
            _ => return Err(integrity()),
        }
        .map_err(|_| integrity())
    }
    fn records_agree(
        &self,
        required: DeclarationId,
        occurrence: SurfaceDeclarationId,
        complete: &definitions::RecordDefinition,
        public: &definitions::RecordDefinition,
    ) -> Result<bool, ModuleSourcesFailure> {
        let DeclarationId::Nominal(record) = required else {
            return Err(integrity());
        };
        let slots = self.headers.field_ids(record).ok_or_else(integrity)?;
        let physical_fields = self
            .binding
            .paired
            .inventory
            .surface
            .record_fields(occurrence)
            .map(|(id, _)| id)
            .collect::<Vec<_>>();
        if physical_fields.len() != public.fields.len() {
            return Err(integrity());
        }
        if complete.copy != public.copy {
            return Ok(false);
        }
        for (raw, definition) in physical_fields.iter().zip(&public.fields) {
            let slot = self.binding.fields[raw.index()].ok_or_else(integrity)?;
            let index = slots
                .iter()
                .position(|id| *id == slot)
                .ok_or_else(integrity)?;
            let actual = complete.fields.get(index).ok_or_else(integrity)?;
            if actual != definition {
                return Ok(false);
            }
        }
        Ok(true)
    }
    fn mismatch(
        &self,
        contract: SurfaceDeclarationId,
        definition: SurfaceDeclarationId,
    ) -> ModuleSourcesFailure {
        let root = self.binding.paired.inventory.declarations()[contract.index()];
        let body = self.binding.paired.inventory.declarations()[definition.index()];
        let error = match root.kind() {
            crate::SurfaceDeclarationKind::Struct | crate::SurfaceDeclarationKind::Enum => {
                DeclarationContractError::MismatchedRepresentation {
                    contract: root.node(),
                    definition: body.node(),
                }
            }
            crate::SurfaceDeclarationKind::Constant | crate::SurfaceDeclarationKind::Static => {
                DeclarationContractError::MismatchedCompileTimeInitializer {
                    contract: root.node(),
                    definition: body.node(),
                }
            }
            _ => DeclarationContractError::MismatchedBody {
                contract: root.node(),
                body: body.node(),
            },
        };
        super::super::module_sources::contract_failure(
            error,
            &self.binding.paired.inventory.surface,
        )
    }
    pub(super) fn project_header_failure(
        &self,
        failure: HeaderFailure,
        occurrence: Option<SurfaceDeclarationId>,
    ) -> ModuleSourcesFailure {
        let HeaderFailure::Rejected(violations) = failure else {
            return integrity();
        };
        let mut codes = self.binding.catalog.sources.codes.as_ref().clone();
        let mut records = self.binding.catalog.sources.records.as_ref().clone();
        let mut enums = self.binding.catalog.sources.enums.as_ref().clone();
        for (raw, header) in &self.binding.catalog.parsed {
            if self.binding.paired.contracts.representative(*raw) != *raw {
                continue;
            }
            if let (Some(DeclarationId::Code(id)), ParsedHeader::Code(header)) =
                (self.binding.declarations[raw.index()], header)
            {
                codes.insert(id, CodeSource::from_header(header));
            }
        }
        if let Some(raw) = occurrence {
            match (
                self.binding.declarations[raw.index()],
                &self.binding.catalog.parsed[&raw],
            ) {
                (Some(DeclarationId::Code(id)), ParsedHeader::Code(header)) => {
                    codes.insert(id, CodeSource::from_header(header));
                }
                (Some(DeclarationId::Nominal(id)), ParsedHeader::Record(header)) => {
                    records.insert(id, header.source.clone());
                }
                (Some(DeclarationId::Nominal(id)), ParsedHeader::Enumeration(header)) => {
                    enums.insert(id, header.source.clone());
                }
                (Some(DeclarationId::Alias(_)), ParsedHeader::Alias(_)) => {}
                _ => return integrity(),
            }
        }
        let trees = self
            .binding
            .paired
            .inventory
            .sources()
            .iter()
            .map(crate::SurfaceSource::syntax)
            .collect::<Vec<_>>();
        ModuleSourcesFailure::Access(projection::project_violations_in(
            &trees,
            &codes,
            &records,
            &enums,
            &self.binding.catalog.sources.aliases,
            violations,
        ))
    }
}

fn project_occurrence<'value>(
    program: &Program,
    required: DeclarationId,
    header: &ParsedHeader,
    scope: HeaderScope<'_>,
    sources: &Sources,
    value: &mut dyn FnMut(AccessCodeId) -> Option<&'value FrozenData>,
) -> Result<Projection<Definition>, ModuleSourcesFailure> {
    match (required, header) {
        (DeclarationId::Code(code), ParsedHeader::Code(header)) => definitions::project_code(
            program,
            scope,
            code,
            &definitions::Recipe::from(header.clone()),
            &CodeSource::from_header(header),
            sources,
            value,
        )
        .map(|p| p.map(Definition::Code)),
        (DeclarationId::Nominal(record), ParsedHeader::Record(header)) => {
            definitions::project_record(program, scope, record, header, sources, value)
                .map(|p| p.map(Definition::Record))
        }
        (DeclarationId::Nominal(enumeration), ParsedHeader::Enumeration(header)) => {
            definitions::project_enum(program, scope, enumeration, header, sources, value)
                .map(|p| p.map(Definition::Enumeration))
        }
        (DeclarationId::Alias(alias), ParsedHeader::Alias(header)) => {
            definitions::project_alias(program, scope, alias, header, sources, value)
                .map(|p| p.map(Definition::Alias))
        }
        _ => return Err(integrity()),
    }
    .map_err(ModuleSourcesFailure::Access)
}
