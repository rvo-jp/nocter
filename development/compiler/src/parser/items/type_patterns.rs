use super::*;
use crate::ast::{BinderRefinementPredicate, GenericParam, GenericParamList, WherePredicate};
use std::collections::HashSet;

impl Parser<'_> {
    pub(super) fn reject_declaration_pattern_prefix(
        &mut self,
        declaration: &str,
    ) -> ParseResult<()> {
        if self.at_punctuation("<") {
            self.error_current(format!(
                "`{declaration}` parameters are declared by its type pattern; remove the prefix `<...>`"
            ));
            return Err(());
        }
        Ok(())
    }

    pub(super) fn declaration_pattern_parameters(
        &mut self,
        types: &[&TypeExpr],
    ) -> ParseResult<GenericParamList> {
        let mut parameters = Vec::new();
        let mut seen = HashSet::new();
        for ty in types {
            self.collect_declaration_pattern_parameters(ty, &mut parameters, &mut seen)?;
        }
        let span = parameters
            .first()
            .zip(parameters.last())
            .map(|(first, last)| self.span(first.name_span.start, last.name_span.end));
        Ok(GenericParamList { span, parameters })
    }

    pub(super) fn classify_declaration_pattern_refinements(
        &mut self,
        requirements: &mut Option<crate::ast::WhereClause>,
        generics: &GenericParamList,
    ) {
        let names = generics
            .parameters
            .iter()
            .map(|parameter| parameter.name.as_str())
            .collect::<HashSet<_>>();
        let Some(clause) = requirements else {
            return;
        };
        for predicate in &mut clause.predicates {
            let WherePredicate::Equality(equality) = predicate else {
                continue;
            };
            let TypeExpr::Reference(left) = &equality.left else {
                continue;
            };
            if !names.contains(left.name.as_str()) {
                continue;
            }
            *predicate = WherePredicate::Refinement(BinderRefinementPredicate {
                span: equality.span,
                equals_span: equality.equals_span,
                name: left.name.clone(),
                name_span: left.span,
                value: equality.right.clone(),
            });
        }
    }

    fn collect_declaration_pattern_parameters(
        &mut self,
        ty: &TypeExpr,
        parameters: &mut Vec<GenericParam>,
        seen: &mut HashSet<String>,
    ) -> ParseResult<()> {
        match ty {
            TypeExpr::Reference(_) => Ok(()),
            TypeExpr::Generic(generic) => {
                for argument in &generic.arguments {
                    self.collect_pattern_slot(argument, parameters, seen)?;
                }
                Ok(())
            }
            TypeExpr::View(view) if !view.is_readwrite => {
                self.collect_pattern_slot(&view.element, parameters, seen)
            }
            _ => Ok(()),
        }
    }

    fn collect_pattern_slot(
        &mut self,
        ty: &TypeExpr,
        parameters: &mut Vec<GenericParam>,
        seen: &mut HashSet<String>,
    ) -> ParseResult<()> {
        let TypeExpr::Reference(reference) = ty else {
            self.error_at(
                ty.span(),
                "declaration pattern arguments must be bare binders; introduce a name here and refine it with `where Name = Type`",
            );
            return Err(());
        };
        if seen.insert(reference.name.clone()) {
            parameters.push(GenericParam {
                span: reference.span,
                name: reference.name.clone(),
                name_span: reference.span,
            });
        }
        Ok(())
    }
}
