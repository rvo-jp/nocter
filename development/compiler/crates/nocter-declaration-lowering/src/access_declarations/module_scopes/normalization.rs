//! Resolve every paired occurrence before loading one define-once semantic header.
use super::{ModuleSourcesFailure, ParsedHeader, ScopedModuleSources, integrity, syntax};
use crate::access_declarations::{
    CodeSource, HeaderProgress,
    definitions::{self, HeaderScope, Projection},
    projection,
};
use crate::{DeclarationContractError, SurfaceDeclarationId};
use nocter_declarations::value_access::{DeclarationId, HeaderFailure, HeaderStep};
use nocter_model::{AccessCodeId, FrozenData};
use std::collections::BTreeMap;

enum Definition {
    Code(definitions::CodeDefinition),
    Record(definitions::RecordDefinition),
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
            for (raw, header) in occurrences {
                let physical = self.binding.paired.inventory.declarations()[raw.index()];
                let scope = HeaderScope {
                    syntax: syntax(&self.binding.paired, physical.source())?,
                    lookup: self.binding.scopes[physical.source().index()],
                };
                let Projection {
                    definition,
                    progress,
                } = match (required, header) {
                    (DeclarationId::Code(code), ParsedHeader::Code(header)) => {
                        definitions::project_code(
                            &program,
                            scope,
                            code,
                            &definitions::Recipe::from(header.clone()),
                            &CodeSource::from_header(header),
                            &self.binding.catalog.sources,
                            &mut value,
                        )
                        .map(|p| Projection {
                            definition: p.definition.map(Definition::Code),
                            progress: p.progress,
                        })
                    }
                    (DeclarationId::Record(record), ParsedHeader::Record(header)) => {
                        definitions::project_record(
                            &program,
                            scope,
                            record,
                            header,
                            &self.binding.catalog.sources,
                            &mut value,
                        )
                        .map(|p| Projection {
                            definition: p.definition.map(Definition::Record),
                            progress: p.progress,
                        })
                    }
                    _ => return Err(integrity()),
                }
                .map_err(ModuleSourcesFailure::Access)?;
                if !progress.violations.is_empty() {
                    return Err(self.project_header_failure(
                        HeaderFailure::Rejected(progress.violations),
                        Some(*raw),
                    ));
                }
                missing.extend(progress.missing);
                if let Some(definition) = definition {
                    projected.insert(*raw, definition);
                }
            }
            if !missing.is_empty() {
                missing.sort_unstable();
                missing.dedup();
                return Ok(HeaderProgress::RequireValues(missing.into()));
            }
            self.define_agreed(required, projected)?;
        }
        self.headers
            .advance(required)
            .map(|step| match step {
                HeaderStep::Ready => HeaderProgress::Ready,
                HeaderStep::Require(headers) => HeaderProgress::RequireHeaders(headers),
            })
            .map_err(|failure| self.project_header_failure(failure, None))
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
            (DeclarationId::Record(id), Definition::Record(definition)) => self
                .headers
                .define_record(id, definition.copy, definition.fields),
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
        let DeclarationId::Record(record) = required else {
            return Err(integrity());
        };
        let slots = self.headers.field_ids(record).ok_or_else(integrity)?;
        let physical_fields = self
            .binding
            .paired
            .inventory
            .declaration_entries()
            .filter(|(_, field)| {
                field.owner() == Some(occurrence)
                    && field.kind() == crate::SurfaceDeclarationKind::Field
            })
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
            crate::SurfaceDeclarationKind::Struct => {
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
                (Some(DeclarationId::Record(id)), ParsedHeader::Record(header)) => {
                    records.insert(id, header.source.clone());
                }
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
            &trees, &codes, &records, violations,
        ))
    }
}
