//! Bind syntax header references through the declaration-owned construction namespace.
use super::{
    CallableTypeSyntax, CodeSource, Failure, TypeSyntax,
    headers::{AliasHeader, Header, RecordHeader},
    source::Sources,
    text,
};
use nocter_declarations::value_access::{
    DataReference, DeclarationId, DeclarationRule, DeclarationViolation, FieldDefinition,
    FunctionDefinition, HeaderPosition, HeaderPublication, ParameterDefinition, Program, ScopeId,
    TupleData, ValueDefinition,
};
use nocter_model::{
    AccessCodeId, ConstantValue, FrozenData, FrozenNode, NominalTypeId, Symbol, TypeAliasId,
    ValueInputOperation,
};
use nocter_syntax::{BoundSyntax, SyntaxOrigin, SyntaxToken};
use std::collections::BTreeMap;

/// Raw type/operation recipe only; names and source positions remain in the source catalog.
#[derive(Debug)]
pub(super) struct Recipe {
    kind: nocter_declarations::value_access::CodeKind,
    parameters: Box<[(ValueInputOperation, TypeSyntax)]>,
    result: TypeSyntax,
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

#[derive(Clone, Copy)]
pub(super) struct HeaderScope<'syntax> {
    pub syntax: BoundSyntax<'syntax>,
    pub lookup: ScopeId,
}
#[derive(Debug, Eq, PartialEq)]
pub(super) struct CodeDefinition {
    pub kind: nocter_declarations::value_access::CodeKind,
    pub parameters: Vec<ParameterDefinition>,
    pub result: ValueDefinition,
}
#[derive(Debug, Eq, PartialEq)]
pub(super) struct RecordDefinition {
    pub copy: bool,
    pub fields: Vec<FieldDefinition>,
}
pub(super) struct Projection<T> {
    pub definition: Option<T>,
    pub progress: Progress,
}
impl<T> Projection<T> {
    pub fn map<U>(self, map: impl FnOnce(T) -> U) -> Projection<U> {
        Projection {
            definition: self.definition.map(map),
            progress: self.progress,
        }
    }
}
#[derive(Default)]
pub(super) struct Progress {
    pub violations: Vec<DeclarationViolation>,
    pub missing: Vec<AccessCodeId>,
    pub missing_aliases: Vec<TypeAliasId>,
}
struct Definitions<'a, 's, 'v> {
    program: &'a Program,
    scope: HeaderScope<'s>,
    sources: &'a Sources,
    values: &'a mut dyn FnMut(AccessCodeId) -> Option<&'v FrozenData>,
    progress: Progress,
}
pub(super) fn define<'v>(
    publication: &mut HeaderPublication,
    scope: HeaderScope<'_>,
    id: DeclarationId,
    functions: &BTreeMap<AccessCodeId, Recipe>,
    sources: &Sources,
    values: &mut dyn FnMut(AccessCodeId) -> Option<&'v FrozenData>,
    records: &BTreeMap<NominalTypeId, RecordHeader>,
) -> Result<Progress, Failure> {
    let program = publication.snapshot();
    match id {
        DeclarationId::Code(id) => {
            let projected = project_code(
                &program,
                scope,
                id,
                functions.get(&id).ok_or(Failure::InconsistentSyntax)?,
                sources.codes.get(&id).ok_or(Failure::InconsistentSyntax)?,
                sources,
                values,
            )?;
            if let Some(definition) = projected.definition {
                publication
                    .define(
                        id,
                        definition.kind,
                        definition.parameters,
                        definition.result,
                    )
                    .map_err(|_| Failure::InconsistentSyntax)?;
            }
            Ok(projected.progress)
        }
        DeclarationId::Nominal(id) => {
            let projected = project_record(
                &program,
                scope,
                id,
                records.get(&id).ok_or(Failure::InconsistentSyntax)?,
                sources,
                values,
            )?;
            if let Some(definition) = projected.definition {
                publication
                    .define_record(id, definition.copy, definition.fields)
                    .map_err(|_| Failure::InconsistentSyntax)?;
            }
            Ok(projected.progress)
        }
        DeclarationId::Alias(_) => Err(Failure::InconsistentSyntax),
    }
}
pub(super) fn project_code<'v>(
    program: &Program,
    scope: HeaderScope<'_>,
    id: AccessCodeId,
    header: &Recipe,
    source: &CodeSource,
    sources: &Sources,
    values: &mut dyn FnMut(AccessCodeId) -> Option<&'v FrozenData>,
) -> Result<Projection<CodeDefinition>, Failure> {
    let mut definitions = Definitions {
        program,
        scope,
        sources,
        values,
        progress: Progress::default(),
    };
    let definition = definitions.function(id, header, source)?;
    Ok(Projection {
        definition,
        progress: definitions.progress,
    })
}
pub(super) fn project_record<'v>(
    program: &Program,
    scope: HeaderScope<'_>,
    id: NominalTypeId,
    header: &RecordHeader,
    sources: &Sources,
    values: &mut dyn FnMut(AccessCodeId) -> Option<&'v FrozenData>,
) -> Result<Projection<RecordDefinition>, Failure> {
    let mut definitions = Definitions {
        program,
        scope,
        sources,
        values,
        progress: Progress::default(),
    };
    let definition = definitions.record(id, header)?;
    Ok(Projection {
        definition,
        progress: definitions.progress,
    })
}
pub(super) fn project_alias<'v>(
    program: &Program,
    scope: HeaderScope<'_>,
    alias: TypeAliasId,
    header: &AliasHeader,
    sources: &Sources,
    values: &mut dyn FnMut(AccessCodeId) -> Option<&'v FrozenData>,
) -> Result<Projection<DataReference>, Failure> {
    let mut definitions = Definitions {
        program,
        scope,
        sources,
        values,
        progress: Progress::default(),
    };
    let definition = definitions.data_type(&header.target, alias.into(), HeaderPosition::Result)?;
    Ok(Projection {
        definition,
        progress: definitions.progress,
    })
}
impl Definitions<'_, '_, '_> {
    fn symbol(&self, token: SyntaxToken) -> Result<Symbol, Failure> {
        nocter_source_index::SourceOrigin::from_token(self.scope.syntax.tree(), token)
            .map_err(|_| Failure::InconsistentSyntax)?;
        self.program
            .symbols()
            .get(text(self.scope.syntax, token))
            .ok_or(Failure::InconsistentSyntax)
    }
    fn data(
        &mut self,
        token: SyntaxToken,
        declaration: DeclarationId,
        position: HeaderPosition,
    ) -> Result<Option<DataReference>, Failure> {
        let resolved = self
            .program
            .resolve_data_in(self.scope.lookup, self.symbol(token)?)
            .map(Some);
        self.selected_data(declaration, position, resolved)
    }
    fn selected_data(
        &mut self,
        declaration: DeclarationId,
        position: HeaderPosition,
        resolved: Result<Option<DataReference>, DeclarationRule>,
    ) -> Result<Option<DataReference>, Failure> {
        match resolved {
            Ok(Some(data)) => Ok(Some(data)),
            Ok(None) => Err(Failure::InconsistentSyntax),
            Err(DeclarationRule::UnreadyAlias { alias }) => {
                self.progress.missing_aliases.push(alias);
                Ok(None)
            }
            Err(rule) => {
                self.progress.violations.push(DeclarationViolation {
                    declaration,
                    position,
                    rule,
                });
                Ok(None)
            }
        }
    }
    fn data_path(
        &mut self,
        path: &[SyntaxToken],
        owner: DeclarationId,
        position: HeaderPosition,
    ) -> Result<Option<DataReference>, Failure> {
        let [token] = path else {
            let names = path
                .iter()
                .map(|&token| self.symbol(token).map(Some))
                .collect::<Result<Vec<_>, _>>()?;
            let resolved = self
                .program
                .declaration_path_in(
                    self.scope.lookup,
                    nocter_declarations::value_access::Namespace::Type,
                    &names,
                )
                .map_err(|error| DeclarationRule::QualifiedDataName { error })
                .and_then(|target| self.program.data_reference_for(target));
            return self.selected_data(owner, position, resolved);
        };
        self.data(*token, owner, position)
    }
    fn data_type(
        &mut self,
        ty: &TypeSyntax,
        owner: DeclarationId,
        position: HeaderPosition,
    ) -> Result<Option<DataReference>, Failure> {
        match ty {
            TypeSyntax::Callable(contract) => self.callable_data(contract, owner, position),
            TypeSyntax::Named(path) => self.data_path(path, owner, position),
            TypeSyntax::Future { output, .. } => Ok(self
                .data_type(output, owner, position)?
                .map(|output| DataReference::Future(std::sync::Arc::new(output)))),
            TypeSyntax::Optional { payload, layers } => self
                .data_type(payload, owner, position)?
                .map(|payload| {
                    DataReference::optional(payload, *layers).ok_or(Failure::InconsistentSyntax)
                })
                .transpose(),
            TypeSyntax::Fallible { payload, .. } => Ok(self
                .data_type(payload, owner, position)?
                .map(|payload| DataReference::Fallible(std::sync::Arc::new(payload)))),
            TypeSyntax::Array { element, length } => {
                nocter_source_index::SourceOrigin::from_node(self.scope.syntax.tree(), *length)
                    .map_err(|_| Failure::InconsistentSyntax)?;
                let element = self.data_type(element, owner, position)?;
                let code = self
                    .sources
                    .expression(*length)
                    .ok_or(Failure::InconsistentSyntax)?;
                let Some(value) = (self.values)(code) else {
                    self.progress.missing.push(code);
                    return Ok(None);
                };
                let FrozenNode::Scalar(ConstantValue::Integer(length)) = value.node() else {
                    return Err(Failure::InconsistentSyntax);
                };
                let length = u64::try_from(*length).map_err(|_| Failure::InconsistentSyntax)?;
                Ok(element.map(|element| DataReference::Array {
                    element: std::sync::Arc::new(element),
                    length,
                }))
            }
            TypeSyntax::Tuple(fields) => {
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

    fn callable_data(
        &mut self,
        contract: &CallableTypeSyntax,
        owner: DeclarationId,
        position: HeaderPosition,
    ) -> Result<Option<DataReference>, Failure> {
        let syntax = self.scope.syntax;
        let reject = |rule, primary, related| {
            super::projection::authored_violation(
                syntax,
                DeclarationViolation {
                    declaration: owner,
                    position,
                    rule,
                },
                primary,
                related,
            )
        };
        if let Some((first, repeated)) = contract.repeated_input_name(syntax) {
            return Err(reject(
                DeclarationRule::DuplicateCallableInputName,
                SyntaxOrigin::Token(repeated),
                Some(SyntaxOrigin::Token(first)),
            ));
        }
        let inputs = contract
            .inputs
            .iter()
            .map(|input| {
                let Some(data) = self.data_type(&input.data, owner, position)? else {
                    return Ok(None);
                };
                if let Some(binding) = &input.binding
                    && !binding.shape.matches_data_reference(&data)
                {
                    return Err(reject(
                        DeclarationRule::ParameterPatternMismatch,
                        SyntaxOrigin::Node(binding.origin),
                        None,
                    ));
                }
                Ok(Some((input.operation, data)))
            })
            .collect::<Result<Vec<_>, Failure>>()?
            .into_iter()
            .collect::<Option<Box<[_]>>>();
        let result = self.data_type(&contract.result, owner, position)?;
        Ok(inputs.zip(result).map(|(inputs, result)| {
            DataReference::Function(Box::new(FunctionDefinition {
                invocation: contract.invocation,
                inputs,
                result,
            }))
        }))
    }
    fn record(
        &mut self,
        id: NominalTypeId,
        header: &RecordHeader,
    ) -> Result<Option<RecordDefinition>, Failure> {
        self.symbol(header.source.name)?;
        let fields = header
            .fields
            .iter()
            .enumerate()
            .map(|(index, data)| {
                let Some(data) = self.data_type(
                    data,
                    DeclarationId::Nominal(id),
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
        Ok(fields.map(|fields| RecordDefinition {
            copy: header.copy,
            fields,
        }))
    }
    fn value(
        &mut self,
        ty: &TypeSyntax,
        owner: DeclarationId,
        position: HeaderPosition,
    ) -> Result<Option<ValueDefinition>, Failure> {
        Ok(self.data_type(ty, owner, position)?.map(|data| match data {
            DataReference::Function(contract) => ValueDefinition::Function(*contract),
            data => ValueDefinition::Data(data),
        }))
    }
    fn function(
        &mut self,
        id: AccessCodeId,
        header: &Recipe,
        source: &CodeSource,
    ) -> Result<Option<CodeDefinition>, Failure> {
        self.symbol(source.name().ok_or(Failure::InconsistentSyntax)?)?;
        let bindings = source
            .parameters()
            .iter()
            .map(|parameter| {
                parameter.binding.try_map(&mut |origin| match origin {
                    SyntaxOrigin::Token(token) => self.symbol(*token),
                    SyntaxOrigin::Node(_) => Err(Failure::InconsistentSyntax),
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        self.progress.violations.extend(
            self.program
                .parameter_name_violations_in(self.scope.lookup, id, &bindings)
                .map_err(|_| Failure::InconsistentSyntax)?,
        );
        if bindings.len() != header.parameters.len() {
            return Err(Failure::InconsistentSyntax);
        }
        let mut parameters = Vec::new();
        let mut complete = true;
        for (index, ((operation, parameter), binding)) in
            header.parameters.iter().zip(bindings).enumerate()
        {
            let owner = DeclarationId::Code(id);
            let position = HeaderPosition::Parameter(index);
            let data = self.value(parameter, owner, position)?;
            if let Some(data) = data {
                parameters.push(ParameterDefinition {
                    binding,
                    data,
                    operation: *operation,
                });
            } else {
                complete = false;
            }
        }

        let result = self.value(
            &header.result,
            DeclarationId::Code(id),
            HeaderPosition::Result,
        )?;
        Ok(result.filter(|_| complete).map(|result| CodeDefinition {
            kind: header.kind,
            parameters,
            result,
        }))
    }
}
