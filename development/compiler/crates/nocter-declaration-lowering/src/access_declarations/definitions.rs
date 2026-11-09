//! Bind syntax header references through the declaration-owned construction namespace.
use super::{
    Failure, ValueTypeSyntax,
    headers::{Header, RecordHeader},
    text,
};
use nocter_declarations::value_access::{
    Builder, DataReference, DeclarationId, DeclarationViolation, FieldDefinition,
    FunctionDefinition, HeaderPosition, ParameterDefinition, ValueDefinition,
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
    fn value(
        &mut self,
        ty: &ValueTypeSyntax,
        owner: DeclarationId,
        position: HeaderPosition,
    ) -> Result<Option<ValueDefinition>, Failure> {
        Ok(match ty {
            ValueTypeSyntax::Data(token) => self
                .data(*token, owner, position)?
                .map(ValueDefinition::Data),
            ValueTypeSyntax::Function(contract) => {
                let inputs = contract
                    .inputs
                    .iter()
                    .map(|&(operation, token)| {
                        Ok(self
                            .data(token, owner, position)?
                            .map(|data| (operation, data)))
                    })
                    .collect::<Result<Option<Vec<_>>, Failure>>()?;
                let result = self.data(contract.result, owner, position)?;
                match (inputs, result) {
                    (Some(inputs), Some(result)) => {
                        Some(ValueDefinition::Function(FunctionDefinition {
                            invocation: contract.invocation,
                            inputs: inputs.into_boxed_slice(),
                            result,
                        }))
                    }
                    _ => None,
                }
            }
        })
    }
    fn function(&mut self, id: AccessCallableId, header: &Header) -> Result<(), Failure> {
        let mut parameters = Vec::new();
        let mut complete = true;
        for (index, parameter) in header.parameters.iter().enumerate() {
            let owner = DeclarationId::Callable(id);
            let position = HeaderPosition::Parameter(index);
            let data = self.value(&parameter.data, owner, position)?;
            if let Some(data) = data {
                parameters.push(ParameterDefinition {
                    name: self.symbol(parameter.name)?,
                    data,
                    operation: parameter.operation,
                });
            } else {
                complete = false;
            }
        }

        if let Some(data) = self.value(
            &header.result_data,
            DeclarationId::Callable(id),
            HeaderPosition::Result,
        )? && complete
        {
            self.builder
                .define(id, header.compile_time, parameters, data)
                .map_err(|_| Failure::InconsistentSyntax)?;
        }
        Ok(())
    }
}
