use super::*;
use crate::typecheck::copyability::non_copy_owned_type_kind;

pub(crate) fn collect_typecheck_facts(ast: &AstFile, resolved: &ResolveOutput) -> TypecheckFacts {
    let mut collector = TypecheckFactCollector {
        resolved,
        facts: TypecheckFacts::default(),
        generic_parameters: Vec::new(),
        associated_types: Vec::new(),
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
    generic_parameters: Vec<(String, ByteSpan)>,
    associated_types: Vec<(String, ByteSpan)>,
}

impl TypecheckFactCollector<'_> {
    fn with_generic_scope(&mut self, generics: &GenericParamList, collect: impl FnOnce(&mut Self)) {
        let previous_len = self.generic_parameters.len();
        for parameter in &generics.parameters {
            self.generic_parameters
                .push((parameter.name.clone(), parameter.name_span));
            if !self
                .facts
                .generic_parameter_declarations
                .iter()
                .any(|existing| existing.span == parameter.name_span)
            {
                self.facts
                    .generic_parameter_declarations
                    .push(GenericParameterFact {
                        name: parameter.name.clone(),
                        span: parameter.name_span,
                        is_copy: false,
                        bounds: Vec::new(),
                    });
            }
        }
        collect(self);
        self.generic_parameters.truncate(previous_len);
    }

    fn generic_parameter_declaration(&self, name: &str) -> Option<ByteSpan> {
        self.generic_parameters
            .iter()
            .rev()
            .find_map(|(parameter, span)| (parameter == name).then_some(*span))
    }

    fn with_associated_type_scope(
        &mut self,
        associated_types: impl IntoIterator<Item = (String, ByteSpan)>,
        collect: impl FnOnce(&mut Self),
    ) {
        let previous_len = self.associated_types.len();
        self.associated_types.extend(associated_types);
        collect(self);
        self.associated_types.truncate(previous_len);
    }

    fn associated_type_declaration(&self, name: &str) -> Option<ByteSpan> {
        self.associated_types
            .iter()
            .rev()
            .find_map(|(associated, span)| (associated == name).then_some(*span))
    }
}

mod bodies;
mod expected;
mod expressions;
mod records;
mod signatures;
mod specializations;
mod statements;
