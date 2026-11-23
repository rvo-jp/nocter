//! Reconcile actual required results with existing metadata; never intern another type or value.
use super::{
    CodeSource, Context, SourceMismatch,
    definitions::{self, HeaderScope, Projection},
    headers::{self, EnumHeader, Header, RecordHeader},
    module_scopes::ParsedHeader,
};
use nocter_declarations::value_access::{DeclarationId, Program};
use nocter_model::{AccessCodeId, FrozenData, NominalTypeId};

impl Context<'_> {
    /// Require source header counts to reproduce this snapshot's established type identities.
    /// # Errors
    /// Rejects incomplete metadata, missing actual values or a mismatching source/type projection.
    /// This is consistency evidence only; it neither evaluates source nor certifies body safety.
    pub fn verify_header_values<'value>(
        &self,
        mut values: impl FnMut(AccessCodeId) -> Option<&'value FrozenData>,
    ) -> Result<(), SourceMismatch> {
        if !self.program().headers_complete() {
            return Err(SourceMismatch::InconsistentSyntax);
        }
        if let Some(binding) = self.module {
            for occurrence in binding.header_occurrences() {
                let (id, header, scope) = occurrence?;
                match (id, header) {
                    (DeclarationId::Code(id), ParsedHeader::Code(header)) => {
                        self.verify_code_header(id, header.clone(), scope, &mut values)?;
                    }
                    (DeclarationId::Nominal(id), ParsedHeader::Record(header)) => {
                        self.verify_record_header(id, header, scope, &mut values)?;
                    }
                    (DeclarationId::Nominal(id), ParsedHeader::Enumeration(header)) => {
                        self.verify_enum_header(id, header, scope, &mut values)?;
                    }
                    (DeclarationId::Alias(id), ParsedHeader::Alias(header)) => {
                        let actual = complete(
                            definitions::project_alias(
                                self.program(),
                                scope,
                                id,
                                header,
                                &self.sources,
                                &mut values,
                            )
                            .map_err(|_| SourceMismatch::InconsistentSyntax)?,
                        )?;
                        let alias = self
                            .program()
                            .alias(id)
                            .ok_or(SourceMismatch::InconsistentSyntax)?;
                        if self.program().existing_data_reference(&actual) != Some(alias.data()) {
                            return Err(SourceMismatch::InconsistentSyntax);
                        }
                    }
                    (
                        DeclarationId::Code(_),
                        ParsedHeader::Test { .. } | ParsedHeader::Destructor { .. },
                    ) => {}
                    _ => return Err(SourceMismatch::InconsistentSyntax),
                }
            }
            return Ok(());
        }

        let scope = HeaderScope {
            syntax: self.syntax,
            lookup: self.lookup,
        };
        let (functions, records) =
            headers::collect(scope.syntax).map_err(|_| SourceMismatch::InconsistentSyntax)?;
        let name = |token| {
            self.program()
                .symbols()
                .get(super::text(scope.syntax, token))
        };
        for header in functions {
            let id = name(header.name)
                .and_then(|name| self.program().resolve_code_in(scope.lookup, name))
                .ok_or(SourceMismatch::InconsistentSyntax)?;
            self.verify_code_header(id, header, scope, &mut values)?;
        }
        for header in records {
            let id = name(header.source.name)
                .and_then(|name| self.program().resolve_nominal_in(scope.lookup, name))
                .ok_or(SourceMismatch::InconsistentSyntax)?;
            self.verify_record_header(id, &header, scope, &mut values)?;
        }
        Ok(())
    }

    fn verify_code_header<'value>(
        &self,
        id: AccessCodeId,
        header: Header,
        scope: HeaderScope<'_>,
        values: &mut dyn FnMut(AccessCodeId) -> Option<&'value FrozenData>,
    ) -> Result<(), SourceMismatch> {
        let source = CodeSource::from_header(&header);
        let actual = complete(
            definitions::project_code(
                self.program(),
                scope,
                id,
                &definitions::Recipe::from(header),
                &source,
                &self.sources,
                values,
            )
            .map_err(|_| SourceMismatch::InconsistentSyntax)?,
        )?;
        if !matches_code(self.program(), id, &actual) {
            return Err(SourceMismatch::InconsistentSyntax);
        }
        Ok(())
    }

    fn verify_record_header<'value>(
        &self,
        id: NominalTypeId,
        header: &RecordHeader,
        scope: HeaderScope<'_>,
        values: &mut dyn FnMut(AccessCodeId) -> Option<&'value FrozenData>,
    ) -> Result<(), SourceMismatch> {
        let actual = complete(
            definitions::project_record(self.program(), scope, id, header, &self.sources, values)
                .map_err(|_| SourceMismatch::InconsistentSyntax)?,
        )?;
        let record = self
            .program()
            .record(id)
            .ok_or(SourceMismatch::InconsistentSyntax)?;
        if record.copyable() != actual.copy {
            return Err(SourceMismatch::InconsistentSyntax);
        }
        for field in actual.fields {
            let id = record
                .resolve_field(field.name)
                .ok_or(SourceMismatch::InconsistentSyntax)?;
            let slot = self
                .program()
                .fields()
                .get(id)
                .ok_or(SourceMismatch::InconsistentSyntax)?;
            if self
                .program()
                .existing_field_data_reference(id, &field.data)
                != Some(slot.data())
            {
                return Err(SourceMismatch::InconsistentSyntax);
            }
        }
        Ok(())
    }

    fn verify_enum_header<'value>(
        &self,
        id: NominalTypeId,
        header: &EnumHeader,
        scope: HeaderScope<'_>,
        values: &mut dyn FnMut(AccessCodeId) -> Option<&'value FrozenData>,
    ) -> Result<(), SourceMismatch> {
        let program = self.program();
        let enumeration = program
            .enumeration(id)
            .ok_or(SourceMismatch::InconsistentSyntax)?;
        let checked = complete(
            definitions::project_enum(program, scope, id, header, &self.sources, values)
                .map_err(|_| SourceMismatch::InconsistentSyntax)?,
        )?;
        let Some(variants) = checked.variants else {
            return if enumeration.copyable() == checked.copy {
                Ok(())
            } else {
                Err(SourceMismatch::InconsistentSyntax)
            };
        };
        if (checked.copy || enumeration.has_payload()) && checked.copy != enumeration.copyable() {
            return Err(SourceMismatch::InconsistentSyntax);
        }
        if enumeration.variants().len() != variants.len() {
            return Err(SourceMismatch::InconsistentSyntax);
        }
        for (position, (&variant_id, source)) in
            enumeration.variants().iter().zip(variants).enumerate()
        {
            let variant = program
                .variants()
                .get(variant_id)
                .ok_or(SourceMismatch::InconsistentSyntax)?;
            if variant.owner() != id
                || variant.position() != position
                || variant.name() != source.name
                || variant.fields().len() != source.fields.len()
            {
                return Err(SourceMismatch::InconsistentSyntax);
            }
            for (position, (&id, supplied)) in
                variant.fields().iter().zip(source.fields).enumerate()
            {
                let field = program
                    .fields()
                    .get(id)
                    .ok_or(SourceMismatch::InconsistentSyntax)?;
                if field.owner() != variant.owner()
                    || field.variant() != Some(variant_id)
                    || field.position() != position
                    || field.name() != supplied.name
                    || program.existing_field_data_reference(id, &supplied.data)
                        != Some(field.data())
                {
                    return Err(SourceMismatch::InconsistentSyntax);
                }
            }
        }
        Ok(())
    }
}
fn complete<T>(projection: Projection<T>) -> Result<T, SourceMismatch> {
    if !projection.progress.missing.is_empty()
        || !projection.progress.missing_aliases.is_empty()
        || !projection.progress.violations.is_empty()
    {
        return Err(SourceMismatch::InconsistentSyntax);
    }
    projection
        .definition
        .ok_or(SourceMismatch::InconsistentSyntax)
}
fn matches_code(
    program: &Program,
    id: AccessCodeId,
    expected: &definitions::CodeDefinition,
) -> bool {
    let Some(code) = program.codes().get(id) else {
        return false;
    };
    code.kind() == expected.kind
        && code.parameters().len() == expected.parameters.len()
        && code
            .parameters()
            .iter()
            .zip(&expected.parameters)
            .all(|(id, expected)| {
                program.parameters().get(*id).is_some_and(|parameter| {
                    parameter.binding() == &expected.binding
                        && parameter.operation() == expected.operation
                        && program.existing_input_value_reference(*id, &expected.data)
                            == Some(parameter.data())
                })
            })
        && program
            .existing_result_value_reference(id, &expected.result)
            .is_some_and(|authored| program.code_body_result(id) == Some(authored))
}
