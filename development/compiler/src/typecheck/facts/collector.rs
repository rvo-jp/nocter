use super::*;
use crate::typecheck::copyability::non_copy_owned_type_kind;

pub(crate) fn collect_typecheck_facts(ast: &AstFile, resolved: &ResolveOutput) -> TypecheckFacts {
    let mut collector = TypecheckFactCollector {
        resolved,
        facts: TypecheckFacts::default(),
    };

    for item in &ast.items {
        collector.collect_item_signature_type_references(item);
    }
    for item in &ast.items {
        collector.collect_item_body_facts(item);
    }

    collector.facts
}

struct TypecheckFactCollector<'a> {
    resolved: &'a ResolveOutput,
    facts: TypecheckFacts,
}

mod bodies;
mod expected;
mod expressions;
mod records;
mod signatures;
mod specializations;
mod statements;
