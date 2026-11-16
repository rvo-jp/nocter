//! Actual body, invocation and static access payloads are independent of signature publication.
use crate::value_access::Failure;
use nocter_declaration_lowering::access_declarations::CodeSource;
use nocter_declarations::value_access::{
    AggregateForm, Code, FunctionConstraint, PayloadSchema, Program,
};
use nocter_model::TypeId;
use nocter_syntax::SyntaxOrigin;
use std::collections::BTreeSet;

pub(super) fn body(program: &Program, code: &Code, source: &CodeSource) -> Result<(), Failure> {
    let mut pending = vec![(code.result(), SyntaxOrigin::Node(source.result_origin()))];
    for (position, &parameter) in code.parameters().iter().enumerate().rev() {
        let data = program
            .parameters()
            .get(parameter)
            .ok_or(Failure::InconsistentSyntax)?
            .data();
        let syntax = source
            .parameter_type(position)
            .ok_or(Failure::InconsistentSyntax)?;
        pending.push((data, SyntaxOrigin::Node(syntax)));
    }
    require_roots(program, pending)
}

pub(in crate::value_access) fn invocation(
    program: &Program,
    contract: &FunctionConstraint,
    origin: SyntaxOrigin,
) -> Result<(), Failure> {
    require_roots(
        program,
        std::iter::once((contract.result, origin)).chain(
            contract
                .inputs
                .iter()
                .rev()
                .map(|input| (input.data, origin)),
        ),
    )
}

pub(in crate::value_access) fn data(
    program: &Program,
    data: TypeId,
    origin: SyntaxOrigin,
) -> Result<(), Failure> {
    require_roots(program, [(data, origin)])
}

fn require_roots(
    program: &Program,
    roots: impl IntoIterator<Item = (TypeId, SyntaxOrigin)>,
) -> Result<(), Failure> {
    let mut pending = roots.into_iter().collect::<Vec<_>>();
    let mut seen = BTreeSet::new();
    while let Some((data, origin)) = pending.pop() {
        if !seen.insert(data) {
            continue;
        }
        if let Some(record) = program.record_identity(data) {
            if !program.header_ready(record) {
                return Err(Failure::RequiredHeader {
                    declaration: record.into(),
                    origin,
                });
            }
            // Publication already closed this record's positive payload dependencies.
            continue;
        }
        match program
            .data_schema(data)
            .ok_or(Failure::InconsistentSyntax)?
        {
            PayloadSchema::Builtin(_) | PayloadSchema::Function(_) => {}
            PayloadSchema::Optional(payload) | PayloadSchema::Fallible(payload) => {
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
    Ok(())
}
