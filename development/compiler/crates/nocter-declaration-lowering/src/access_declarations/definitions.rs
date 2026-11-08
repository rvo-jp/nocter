//! Bind syntax header references through the declaration-owned construction namespace.
use super::{
    Failure,
    headers::{Header, RecordHeader},
    text,
};
use nocter_declarations::value_access::{
    Builder, DataReference, DeclarationId, DeclarationViolation, FieldDefinition, HeaderPosition,
    ParameterDefinition,
};
use nocter_model::{AccessCallableId, NominalTypeId, Symbol, SymbolTable};
use nocter_syntax::{BoundSyntax, SyntaxToken};
use std::collections::BTreeMap;

struct Definitions<'a, 's> {
    builder: &'a mut Builder,
    syntax: BoundSyntax<'s>,
    symbols: &'a SymbolTable,
    violations: Vec<DeclarationViolation>,
}
pub(super) fn define(
    builder: &mut Builder,
    syntax: BoundSyntax<'_>,
    symbols: &SymbolTable,
    functions: &BTreeMap<AccessCallableId, Header>,
    records: &BTreeMap<NominalTypeId, RecordHeader>,
) -> Result<Vec<DeclarationViolation>, Failure> {
    let mut definitions = Definitions {
        builder,
        syntax,
        symbols,
        violations: Vec::new(),
    };
    for (&id, header) in records {
        definitions.record(id, header)?;
    }
    for (&id, header) in functions {
        definitions.function(id, header)?;
    }
    Ok(definitions.violations)
}
impl Definitions<'_, '_> {
    fn symbol(&self, token: SyntaxToken) -> Result<Symbol, Failure> {
        self.symbols
            .get(text(self.syntax, token))
            .ok_or(Failure::InconsistentSyntax)
    }
    fn data(
        &mut self,
        token: SyntaxToken,
        declaration: DeclarationId,
        position: HeaderPosition,
    ) -> Result<Option<DataReference>, Failure> {
        match self.builder.resolve_data(self.symbol(token)?) {
            Ok(data) => Ok(Some(data)),
            Err(rule) => {
                self.violations.push(DeclarationViolation {
                    declaration,
                    position,
                    rule,
                });
                Ok(None)
            }
        }
    }
    fn record(&mut self, id: NominalTypeId, header: &RecordHeader) -> Result<(), Failure> {
        let fields = header
            .fields
            .iter()
            .enumerate()
            .map(|(index, &(name, data, _))| {
                let Some(data) = self.data(
                    data,
                    DeclarationId::Record(id),
                    HeaderPosition::Field(index),
                )?
                else {
                    return Ok(None);
                };
                Ok(Some(FieldDefinition {
                    name: self.symbol(name)?,
                    data,
                }))
            })
            .collect::<Result<Option<Vec<_>>, Failure>>()?;
        if let Some(fields) = fields {
            self.builder
                .define_record(id, header.copy, fields)
                .map_err(|_| Failure::InconsistentSyntax)?;
        }
        Ok(())
    }
    fn function(&mut self, id: AccessCallableId, header: &Header) -> Result<(), Failure> {
        let mut parameters = Vec::new();
        let mut complete = true;
        for (index, parameter) in header.parameters.iter().enumerate() {
            if let Some(data) = self.data(
                parameter.data,
                DeclarationId::Callable(id),
                HeaderPosition::Parameter(index),
            )? {
                parameters.push(ParameterDefinition {
                    name: self.symbol(parameter.name)?,
                    data,
                    operation: parameter.operation,
                });
            } else {
                complete = false;
            }
        }
        if let Some(data) = self.data(
            header.result_data,
            DeclarationId::Callable(id),
            HeaderPosition::Result,
        )? && complete
        {
            self.builder
                .define(id, parameters, data)
                .map_err(|_| Failure::InconsistentSyntax)?;
        }
        Ok(())
    }
}
