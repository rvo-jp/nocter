//! Bind syntax header references through the declaration-owned construction namespace.
use super::{
    DataTypeSyntax, Failure, ValueTypeSyntax,
    headers::{Header, RecordHeader},
    source::Sources,
    text,
};
use nocter_declarations::value_access::{
    DataReference, DeclarationId, DeclarationViolation, FieldDefinition, FunctionDefinition,
    HeaderPosition, HeaderPublication, ParameterDefinition, TupleData, ValueDefinition,
};
use nocter_model::{
    AccessCodeId, ConstantValue, FrozenData, FrozenNode, NominalTypeId, Symbol, ValueInputOperation,
};
use nocter_syntax::{BoundSyntax, SyntaxToken};
use std::collections::BTreeMap;

/// Raw type/operation recipe only; names and source positions remain in the source catalog.
#[derive(Debug)]
pub(super) struct Recipe {
    kind: nocter_declarations::value_access::CodeKind,
    parameters: Box<[(ValueInputOperation, ValueTypeSyntax)]>,
    result: ValueTypeSyntax,
}
impl From<Header> for Recipe {
    fn from(header: Header) -> Self {
        Self {
            kind: header.kind,
            parameters: header
                .parameters
                .into_iter()
                .map(|p| (p.operation, p.data))
                .collect(),
            result: header.result_data,
        }
    }
}

pub(super) struct Progress {
    pub violations: Vec<DeclarationViolation>,
    pub missing: Vec<AccessCodeId>,
}
struct Definitions<'a, 's, 'v> {
    publication: &'a mut HeaderPublication,
    syntax: BoundSyntax<'s>,
    sources: &'a Sources,
    values: &'a mut dyn FnMut(AccessCodeId) -> Option<&'v FrozenData>,
    violations: Vec<DeclarationViolation>,
    missing: Vec<AccessCodeId>,
}
pub(super) fn define<'v>(
    publication: &mut HeaderPublication,
    syntax: BoundSyntax<'_>,
    id: DeclarationId,
    functions: &BTreeMap<AccessCodeId, Recipe>,
    sources: &Sources,
    values: &mut dyn FnMut(AccessCodeId) -> Option<&'v FrozenData>,
    records: &BTreeMap<NominalTypeId, RecordHeader>,
) -> Result<Progress, Failure> {
    let mut definitions = Definitions {
        publication,
        syntax,
        sources,
        values,
        violations: Vec::new(),
        missing: Vec::new(),
    };
    match id {
        DeclarationId::Record(id) => {
            definitions.record(id, records.get(&id).ok_or(Failure::InconsistentSyntax)?)?;
        }
        DeclarationId::Code(id) => {
            definitions.function(id, functions.get(&id).ok_or(Failure::InconsistentSyntax)?)?;
        }
    }
    Ok(Progress {
        violations: definitions.violations,
        missing: definitions.missing,
    })
}
impl Definitions<'_, '_, '_> {
    fn symbol(&self, token: SyntaxToken) -> Result<Symbol, Failure> {
        self.publication
            .symbols()
            .get(text(self.syntax, token))
            .ok_or(Failure::InconsistentSyntax)
    }
    fn data(
        &mut self,
        token: SyntaxToken,
        declaration: DeclarationId,
        position: HeaderPosition,
    ) -> Result<Option<DataReference>, Failure> {
        match self.publication.resolve_data(self.symbol(token)?) {
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
    fn data_type(
        &mut self,
        ty: &DataTypeSyntax,
        owner: DeclarationId,
        position: HeaderPosition,
    ) -> Result<Option<DataReference>, Failure> {
        match ty {
            DataTypeSyntax::Named(token) => self.data(*token, owner, position),
            DataTypeSyntax::Optional { payload, layers } => Ok(self
                .data_type(payload, owner, position)?
                .map(|payload| DataReference::Optional {
                    payload: Box::new(payload),
                    layers: *layers,
                })),
            DataTypeSyntax::Fallible { payload, .. } => Ok(self
                .data_type(payload, owner, position)?
                .map(|payload| DataReference::Fallible(Box::new(payload)))),
            DataTypeSyntax::Array { element, length } => {
                let element = self.data_type(element, owner, position)?;
                let code = self
                    .sources
                    .expression(*length)
                    .ok_or(Failure::InconsistentSyntax)?;
                let Some(value) = (self.values)(code) else {
                    self.missing.push(code);
                    return Ok(None);
                };
                let FrozenNode::Scalar(ConstantValue::Integer(length)) = value.node() else {
                    return Err(Failure::InconsistentSyntax);
                };
                let length = u64::try_from(*length).map_err(|_| Failure::InconsistentSyntax)?;
                Ok(element.map(|element| DataReference::Array {
                    element: Box::new(element),
                    length,
                }))
            }
            DataTypeSyntax::Tuple(fields) => {
                let fields = fields
                    .iter()
                    .map(|field| self.data_type(field, owner, position))
                    .collect::<Result<Vec<_>, _>>()?
                    .into_iter()
                    .collect::<Option<Vec<_>>>();
                Ok(fields.map(|fields| {
                    let mut fields = fields.into_iter();
                    DataReference::Tuple(TupleData::new(
                        fields.next().expect("tuple arity"),
                        fields.next().expect("tuple arity"),
                        fields,
                    ))
                }))
            }
        }
    }
    fn record(&mut self, id: NominalTypeId, header: &RecordHeader) -> Result<(), Failure> {
        let fields = header
            .fields
            .iter()
            .enumerate()
            .map(|(index, data)| {
                let Some(data) = self.data_type(
                    data,
                    DeclarationId::Record(id),
                    HeaderPosition::Field(index),
                )?
                else {
                    return Ok(None);
                };
                Ok(Some(FieldDefinition {
                    name: self.symbol(header.source.fields[index].name)?,
                    data,
                }))
            })
            .collect::<Result<Vec<_>, Failure>>()?
            .into_iter()
            .collect::<Option<Vec<_>>>();
        if let Some(fields) = fields {
            self.publication
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
            ValueTypeSyntax::Data(data) => self
                .data_type(data, owner, position)?
                .map(ValueDefinition::Data),
            ValueTypeSyntax::Function(contract) => {
                let inputs = contract
                    .inputs
                    .iter()
                    .map(|(operation, data)| {
                        Ok(self
                            .data_type(data, owner, position)?
                            .map(|data| (*operation, data)))
                    })
                    .collect::<Result<Vec<_>, Failure>>()?
                    .into_iter()
                    .collect::<Option<Vec<_>>>();
                let result = self.data_type(&contract.result, owner, position)?;
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
    fn function(&mut self, id: AccessCodeId, header: &Recipe) -> Result<(), Failure> {
        let mut parameters = Vec::new();
        let mut complete = true;
        for (index, (operation, parameter)) in header.parameters.iter().enumerate() {
            let owner = DeclarationId::Code(id);
            let position = HeaderPosition::Parameter(index);
            let data = self.value(parameter, owner, position)?;
            if let Some(data) = data {
                parameters.push(ParameterDefinition {
                    name: self.symbol(
                        *self
                            .sources
                            .codes
                            .get(&id)
                            .and_then(|s| s.parameters().get(index))
                            .ok_or(Failure::InconsistentSyntax)?,
                    )?,
                    data,
                    operation: *operation,
                });
            } else {
                complete = false;
            }
        }

        if let Some(data) = self.value(
            &header.result,
            DeclarationId::Code(id),
            HeaderPosition::Result,
        )? && complete
        {
            self.publication
                .define(id, header.kind, parameters, data)
                .map_err(|_| Failure::InconsistentSyntax)?;
        }
        Ok(())
    }
}
