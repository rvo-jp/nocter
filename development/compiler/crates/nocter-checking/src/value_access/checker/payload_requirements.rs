//! Actual body, invocation and static access payloads are independent of signature publication.
use crate::value_access::{Failure, Rule, diagnostics};
use nocter_declaration_lowering::access_declarations::CodeSource;
use nocter_declarations::value_access::{
    AggregateForm, Code, FunctionConstraint, PayloadSchema, Program,
};
use nocter_model::{NominalTypeId, TypeId};
use nocter_syntax::{BoundSyntax, SyntaxOrigin};
use std::collections::BTreeSet;

pub(super) fn body(
    program: &Program,
    syntax: BoundSyntax<'_>,
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
    require_roots(program, syntax, &pending)
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
    require_roots(program, syntax, &roots)
}

pub(in crate::value_access) fn data(
    program: &Program,
    syntax: BoundSyntax<'_>,
    data: TypeId,
    origin: SyntaxOrigin,
) -> Result<(), Failure> {
    require_roots(program, syntax, &[(data, origin)])
}

fn require_roots(
    program: &Program,
    syntax: BoundSyntax<'_>,
    roots: &[(TypeId, SyntaxOrigin)],
) -> Result<(), Failure> {
    if let Some((record, origin)) = unready(program, roots.iter().copied())? {
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
    unready(program, roots.into_iter().map(|data| (data, ()))).map(|missing| missing.is_none())
}

fn unready<Origin: Copy>(
    program: &Program,
    roots: impl IntoIterator<Item = (TypeId, Origin)>,
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
