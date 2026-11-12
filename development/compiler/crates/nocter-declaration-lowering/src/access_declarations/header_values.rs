//! Reconcile actual required results with existing metadata; never intern another type or value.
use super::{
    CodeSource, Context, SourceMismatch,
    definitions::{self, HeaderScope, Projection},
    headers,
};
use nocter_declarations::value_access::{FunctionConstraint, Program, ValueDefinition};
use nocter_model::{AccessCodeId, FrozenData, TypeId};

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
        let scopes = match self.module {
            Some(binding) => binding.header_scopes()?,
            None => vec![HeaderScope {
                syntax: self.syntax,
                lookup: self.lookup,
            }],
        };
        for scope in scopes {
            let (functions, records) =
                headers::collect(scope.syntax).map_err(|_| SourceMismatch::InconsistentSyntax)?;
            for header in functions {
                let name = self
                    .program()
                    .symbols()
                    .get(super::text(scope.syntax, header.name))
                    .ok_or(SourceMismatch::InconsistentSyntax)?;
                let id = self
                    .program()
                    .resolve_code_in(scope.lookup, name)
                    .ok_or(SourceMismatch::InconsistentSyntax)?;
                let source = CodeSource::from_header(&header);
                let actual = complete(
                    definitions::project_code(
                        self.program(),
                        scope,
                        id,
                        &definitions::Recipe::from(header),
                        &source,
                        &self.sources,
                        &mut values,
                    )
                    .map_err(|_| SourceMismatch::InconsistentSyntax)?,
                )?;
                if !matches_code(self.program(), id, &actual) {
                    return Err(SourceMismatch::InconsistentSyntax);
                }
            }
            for header in records {
                let name = self
                    .program()
                    .symbols()
                    .get(super::text(scope.syntax, header.source.name))
                    .ok_or(SourceMismatch::InconsistentSyntax)?;
                let id = self
                    .program()
                    .resolve_record_in(scope.lookup, name)
                    .ok_or(SourceMismatch::InconsistentSyntax)?;
                let actual = complete(
                    definitions::project_record(
                        self.program(),
                        scope,
                        id,
                        &header,
                        &self.sources,
                        &mut values,
                    )
                    .map_err(|_| SourceMismatch::InconsistentSyntax)?,
                )?;
                let record = self
                    .program()
                    .records()
                    .get(id)
                    .ok_or(SourceMismatch::InconsistentSyntax)?;
                if record.copyable() != actual.copy {
                    return Err(SourceMismatch::InconsistentSyntax);
                }
                for field in actual.fields {
                    let slot = record
                        .resolve_field(field.name)
                        .and_then(|id| self.program().fields().get(id))
                        .ok_or(SourceMismatch::InconsistentSyntax)?;
                    if self.program().existing_data_reference(&field.data) != Some(slot.data()) {
                        return Err(SourceMismatch::InconsistentSyntax);
                    }
                }
            }
        }
        Ok(())
    }
}
fn complete<T>(projection: Projection<T>) -> Result<T, SourceMismatch> {
    if !projection.progress.missing.is_empty() || !projection.progress.violations.is_empty() {
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
                    parameter.name() == expected.name
                        && parameter.operation() == expected.operation
                        && matches_value(
                            program,
                            &expected.data,
                            parameter.data(),
                            parameter.contract(),
                        )
                })
            })
        && matches_value(
            program,
            &expected.result,
            code.result(),
            code.result_contract(),
        )
}
fn matches_value(
    program: &Program,
    expected: &ValueDefinition,
    data: TypeId,
    contract: Option<&FunctionConstraint>,
) -> bool {
    match (expected, contract) {
        (ValueDefinition::Data(expected), None) => {
            program.existing_data_reference(expected) == Some(data)
        }
        (ValueDefinition::Function(expected), Some(actual)) => {
            expected.invocation == actual.invocation
                && expected.inputs.len() == actual.inputs.len()
                && expected
                    .inputs
                    .iter()
                    .zip(&actual.inputs)
                    .all(|((operation, data), actual)| {
                        *operation == actual.operation
                            && program.existing_data_reference(data) == Some(actual.data)
                    })
                && program.existing_data_reference(&expected.result) == Some(actual.result)
        }
        _ => false,
    }
}
