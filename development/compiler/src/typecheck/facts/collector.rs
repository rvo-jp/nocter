use super::*;
use crate::typecheck::copyability::non_copy_owned_type_kind;

pub(in crate::typecheck) fn build_typed_hir(ast: &AstFile, resolved: &ResolveOutput) -> TypedHir {
    let mut collector = TypedHirBuilder {
        resolved,
        facts: TypedHir::new(resolved.semantic_db.clone()),
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

struct TypedHirBuilder<'a> {
    resolved: &'a ResolveOutput,
    facts: TypedHir,
    generic_parameters: Vec<(String, crate::semantic::DefId)>,
    associated_types: Vec<(String, crate::semantic::DefId)>,
}

fn unwrap_group(expression: &Expr) -> &Expr {
    match expression {
        Expr::Group(group) => unwrap_group(&group.expression),
        expression => expression,
    }
}

impl TypedHirBuilder<'_> {
    fn with_generic_scope(&mut self, generics: &GenericParamList, collect: impl FnOnce(&mut Self)) {
        let previous_len = self.generic_parameters.len();
        for parameter in &generics.parameters {
            let definition = self
                .resolved
                .semantic_db
                .definition_at(parameter.name_span)
                .expect("semantic database omitted generic parameter");
            self.generic_parameters
                .push((parameter.name.clone(), definition));
            if !self
                .facts
                .generic_parameter_declarations
                .iter()
                .any(|existing| existing.definition == definition)
            {
                self.facts
                    .generic_parameter_declarations
                    .push(GenericParameterFact {
                        definition,
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

    fn generic_parameter_declaration(&self, name: &str) -> Option<crate::semantic::DefId> {
        self.generic_parameters
            .iter()
            .rev()
            .find_map(|(parameter, definition)| (parameter == name).then_some(*definition))
    }

    fn with_associated_type_scope(
        &mut self,
        associated_types: impl IntoIterator<Item = (String, crate::semantic::DefId)>,
        collect: impl FnOnce(&mut Self),
    ) {
        let previous_len = self.associated_types.len();
        self.associated_types.extend(associated_types);
        collect(self);
        self.associated_types.truncate(previous_len);
    }

    fn associated_type_declaration(&self, name: &str) -> Option<crate::semantic::DefId> {
        self.associated_types
            .iter()
            .rev()
            .find_map(|(associated, definition)| (associated == name).then_some(*definition))
    }
}

mod bodies;
mod expected;
mod expressions;
mod records;
mod signatures;
mod specializations;
mod statements;
