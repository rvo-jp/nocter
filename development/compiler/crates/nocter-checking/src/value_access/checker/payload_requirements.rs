//! Actual body, invocation and static access payloads are independent of signature publication.
use crate::value_access::{Failure, Rule, diagnostics};
use nocter_declaration_lowering::access_declarations::CodeSource;
use nocter_declarations::GenericParameterDomain;
use nocter_declarations::value_access::{
    AggregateForm, Code, FunctionConstraint, PayloadSchema, Program,
};
use nocter_model::{AccessCodeId, NominalTypeId, TypeId, TypeKind, UsizeTerm};
use nocter_syntax::{BoundSyntax, SyntaxOrigin};
use std::collections::BTreeSet;

pub(super) fn body(
    program: &Program,
    syntax: BoundSyntax<'_>,
    owner: AccessCodeId,
    code: &Code,
    result: TypeId,
    source: &CodeSource,
) -> Result<(), Failure> {
    let mut pending = vec![(result, SyntaxOrigin::Node(source.result_origin()))];
    for (position, &parameter) in code.parameters().iter().enumerate().rev() {
        let data = program
            .parameters()
            .get(parameter)
            .ok_or(Failure::InconsistentSyntax)?
            .data();
        let origin = source
            .parameter_type_origin(position)
            .ok_or(Failure::InconsistentSyntax)?;
        pending.push((data, origin));
    }
    require_roots(program, syntax, &pending, Some(owner))
}

pub(in crate::value_access) fn invocation(
    program: &Program,
    syntax: BoundSyntax<'_>,
    contract: &FunctionConstraint,
    origin: SyntaxOrigin,
) -> Result<(), Failure> {
    let roots = std::iter::once((contract.result, origin))
        .chain(
            contract
                .inputs
                .iter()
                .rev()
                .map(|input| (input.data, origin)),
        )
        .collect::<Vec<_>>();
    require_roots(program, syntax, &roots, None)
}

pub(in crate::value_access) fn data(
    program: &Program,
    syntax: BoundSyntax<'_>,
    data: TypeId,
    origin: SyntaxOrigin,
) -> Result<(), Failure> {
    require_roots(program, syntax, &[(data, origin)], None)
}

impl super::Checker<'_, '_> {
    /// Validate a value boundary within the current declaration's lexical type assumptions.
    pub(in crate::value_access) fn require_body_payload(
        &self,
        data: TypeId,
        origin: SyntaxOrigin,
    ) -> Result<(), Failure> {
        require_roots(
            self.program(),
            self.syntax(),
            &[(data, origin)],
            Some(self.code.source_owner()),
        )
    }
}

fn require_roots(
    program: &Program,
    syntax: BoundSyntax<'_>,
    roots: &[(TypeId, SyntaxOrigin)],
    owner: Option<AccessCodeId>,
) -> Result<(), Failure> {
    if let Some((record, origin)) = unready(program, roots.iter().copied(), owner)? {
        return Err(Failure::RequiredHeader {
            declaration: record.into(),
            origin,
        });
    }
    if roots.len() > 1
        && program
            .payload_dependencies_all(roots.iter().map(|&(data, _)| data))
            .is_some_and(|dependencies| !dependencies.requires_future())
    {
        return Ok(());
    }
    for &(data, origin) in roots.iter().rev() {
        if program
            .payload_dependencies(data)
            .is_some_and(|dependencies| dependencies.requires_future())
        {
            let rule = Rule::UnqualifiedFuture;
            return Err(diagnostics::authored_rejection(
                syntax,
                origin,
                rule,
                rule.diagnostic().1,
                Vec::new(),
            ));
        }
    }
    Ok(())
}

/// Metadata-only readiness probe; absence never requests or proves a payload.
pub(in crate::value_access) fn ready(
    program: &Program,
    roots: impl IntoIterator<Item = TypeId>,
) -> Result<bool, Failure> {
    unready(program, roots.into_iter().map(|data| (data, ())), None)
        .map(|missing| missing.is_none())
}

fn unready<Origin: Copy>(
    program: &Program,
    roots: impl IntoIterator<Item = (TypeId, Origin)>,
    owner: Option<AccessCodeId>,
) -> Result<Option<(NominalTypeId, Origin)>, Failure> {
    let mut pending = roots.into_iter().collect::<Vec<_>>();
    let mut seen = BTreeSet::new();
    while let Some((data, origin)) = pending.pop() {
        if !seen.insert(data) {
            continue;
        }
        if let Some(record) = program.nominal_identity(data) {
            if !program.header_ready(record) {
                return Ok(Some((record, origin)));
            }
            // Publication already closed this record's positive payload dependencies.
            continue;
        }
        let symbolic = match program.type_kind(data) {
            Some(TypeKind::GenericParameter(id)) => Some((*id, GenericParameterDomain::Type)),
            Some(TypeKind::FixedArray {
                element,
                length: UsizeTerm::Parameter(id),
            }) => {
                pending.push((*element, origin));
                Some((*id, GenericParameterDomain::UsizeConstant))
            }
            _ => None,
        };
        if let Some((id, domain)) = symbolic {
            if program
                .generic_parameter(id)
                .is_none_or(|slot| Some(slot.owner()) != owner || slot.domain() != domain)
            {
                return Err(Failure::InconsistentSyntax);
            }
            continue;
        }
        match program
            .data_schema(data)
            .ok_or(Failure::InconsistentSyntax)?
        {
            PayloadSchema::Builtin(_)
            | PayloadSchema::Enumeration(_)
            | PayloadSchema::Function(_) => {}
            PayloadSchema::Optional(payload)
            | PayloadSchema::Fallible(payload)
            | PayloadSchema::Future(payload) => {
                pending.push((payload, origin));
            }
            PayloadSchema::Array(array) => {
                if !array.is_empty() {
                    pending.push((array.element(), origin));
                }
            }
            PayloadSchema::Aggregate(aggregate) => {
                let AggregateForm::Tuple(fields) = aggregate.form() else {
                    return Err(Failure::InconsistentSyntax);
                };
                pending.extend(fields.iter().rev().map(|&data| (data, origin)));
            }
        }
    }
    Ok(None)
}
