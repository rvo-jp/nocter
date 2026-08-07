use super::ParseResult;
use crate::ast::{GenericParam, GenericParamList, GenericType, TypeExpr};
use crate::source::ByteSpan;

pub(in crate::parser) fn owner_target_name(target: &TypeExpr) -> Option<(String, ByteSpan)> {
    match target {
        TypeExpr::Reference(reference) => Some((reference.name.clone(), reference.span)),
        TypeExpr::Generic(generic) => Some((generic.name.clone(), generic.name_span)),
        _ => None,
    }
}

pub(in crate::parser) fn owner_target_generics(target: &TypeExpr) -> ParseResult<GenericParamList> {
    let TypeExpr::Generic(GenericType { arguments, .. }) = target else {
        return Ok(GenericParamList::empty());
    };
    let mut parameters = Vec::with_capacity(arguments.len());
    for argument in arguments {
        let TypeExpr::Reference(reference) = argument else {
            return Err(());
        };
        parameters.push(GenericParam {
            span: reference.span,
            name: reference.name.clone(),
            name_span: reference.span,
            bounds: Vec::new(),
        });
    }
    Ok(GenericParamList {
        span: Some(target.span()),
        parameters,
    })
}
