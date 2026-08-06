//! Stable semantic facts shared with compiler analysis and editor features.

use super::TypecheckSource;
use super::copyability::type_expr_is_copy;
use super::provenance::{StorageOrigin, ValueProvenance};
use super::returns::{callable_provenance_summaries, type_expr_contains_borrow_like};
use crate::ast::{ImplMember, Item, TypeExpr};
use crate::resolve::ResolveOutput;
use crate::semantics::{AllocationSource, TrustedDeclarationRole};
use crate::source::ByteSpan;
use std::collections::{BTreeMap, HashMap, HashSet};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct CallableSemanticFacts {
    entries: HashMap<ByteSpan, CallableSemanticFact>,
}

impl CallableSemanticFacts {
    pub(crate) fn get(&self, declaration: ByteSpan) -> Option<&CallableSemanticFact> {
        self.entries.get(&declaration)
    }

    pub(crate) fn entries(&self) -> impl Iterator<Item = (ByteSpan, &CallableSemanticFact)> + '_ {
        self.entries.iter().map(|(span, fact)| (*span, fact))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CallableSemanticFact {
    pub(crate) result: Option<ValueProvenanceFact>,
    pub(crate) needs_current_allocation_context: bool,
    pub(crate) storage_inputs: HashSet<ByteSpan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum StorageOriginFact {
    Static,
    CurrentAllocationContext,
    Input(ByteSpan),
    Scope(ByteSpan),
    Region(ByteSpan),
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ValueProvenanceFact {
    Independent,
    Origins(Vec<StorageOriginFact>),
    Aggregate {
        fallback: Option<Box<ValueProvenanceFact>>,
        fields: BTreeMap<String, ValueProvenanceFact>,
        elements: BTreeMap<usize, ValueProvenanceFact>,
    },
    Fallible {
        success: Option<Box<ValueProvenanceFact>>,
        error: Option<Box<ValueProvenanceFact>>,
    },
}

pub(crate) fn collect_callable_semantic_facts(
    sources: &[TypecheckSource<'_>],
) -> CallableSemanticFacts {
    let summaries = callable_provenance_summaries(sources);
    let mut facts = CallableSemanticFacts::default();
    for source in sources {
        for item in &source.ast.items {
            match item {
                Item::Function(function) => {
                    let declaration = if function.owner.is_some() {
                        function.member_name_span
                    } else {
                        function.name_span
                    };
                    insert_fact(
                        declaration,
                        &function.return_type,
                        &function.parameters.parameters,
                        None,
                        source.resolved,
                        &summaries,
                        &mut facts,
                    );
                }
                Item::Test(test) => {
                    let return_type = test.return_type();
                    insert_fact(
                        test.name_span,
                        &return_type,
                        &[],
                        None,
                        source.resolved,
                        &summaries,
                        &mut facts,
                    );
                }
                Item::Primitive(primitive) => {
                    insert_fact(
                        primitive.name_span,
                        &primitive.return_type,
                        &primitive.parameters.parameters,
                        None,
                        source.resolved,
                        &summaries,
                        &mut facts,
                    );
                }
                Item::Impl(impl_) => {
                    for member in &impl_.members {
                        let ImplMember::Method(method) = member else {
                            continue;
                        };
                        if method.body.is_some() {
                            insert_fact(
                                method.name_span,
                                &method.return_type,
                                &method.parameters.parameters,
                                Some(&method.receiver),
                                source.resolved,
                                &summaries,
                                &mut facts,
                            );
                        }
                    }
                }
                Item::Interface(interface) => {
                    for method in &interface.methods {
                        insert_fact(
                            method.name_span,
                            &method.return_type,
                            &method.parameters.parameters,
                            Some(&method.receiver),
                            source.resolved,
                            &summaries,
                            &mut facts,
                        );
                    }
                }
                Item::Import(_)
                | Item::FromImport(_)
                | Item::TypeAlias(_)
                | Item::Struct(_)
                | Item::Enum(_) => {}
                Item::Construct(construct) => {
                    for (_, function) in construct.functions() {
                        insert_fact(
                            function.member_name_span,
                            &function.return_type,
                            &function.parameters.parameters,
                            None,
                            source.resolved,
                            &summaries,
                            &mut facts,
                        );
                    }
                    for (_, literal) in construct.literals() {
                        insert_fact(
                            literal.span,
                            &literal.return_type,
                            &literal.parameters.parameters,
                            None,
                            source.resolved,
                            &summaries,
                            &mut facts,
                        );
                    }
                }
            }
        }
    }
    facts
}

fn insert_fact(
    declaration: ByteSpan,
    return_type: &TypeExpr,
    parameters: &[crate::ast::Parameter],
    receiver: Option<&crate::ast::MethodReceiver>,
    resolved: &ResolveOutput,
    summaries: &super::provenance::CallableProvenanceSummaries,
    facts: &mut CallableSemanticFacts,
) {
    let callable = super::provenance::CallableId::declared_at(declaration);
    let result = summaries
        .result(callable)
        .map(value_fact)
        .map(normalize_value_fact)
        .map(|fact| normalize_for_return_type(fact, return_type, resolved));
    let needs_current_allocation_context = summaries.needs_current_allocation_context(callable)
        || declaration_uses_current_allocation_context(resolved, declaration);
    let storage_inputs = parameters
        .iter()
        .filter(|parameter| type_expr_carries_storage(&parameter.ty, resolved))
        .map(|parameter| parameter.name_span)
        .chain(receiver.map(|receiver| receiver.name_span))
        .collect();
    facts.entries.insert(
        declaration,
        CallableSemanticFact {
            result,
            needs_current_allocation_context,
            storage_inputs,
        },
    );
}

fn type_expr_carries_storage(ty: &TypeExpr, resolved: &ResolveOutput) -> bool {
    type_expr_contains_borrow_like(ty, resolved, &HashMap::new(), &mut HashSet::new())
        || type_expr_is_copy(ty, resolved) != Some(true)
}

fn declaration_uses_current_allocation_context(
    resolved: &ResolveOutput,
    declaration: ByteSpan,
) -> bool {
    matches!(
        resolved.trusted_declarations.role(declaration),
        Some(TrustedDeclarationRole::CurrentAllocationContext)
            | Some(TrustedDeclarationRole::AllocationOperation {
                source: AllocationSource::CurrentContext,
                ..
            })
    )
}

fn normalize_for_return_type(
    fact: ValueProvenanceFact,
    return_type: &TypeExpr,
    resolved: &ResolveOutput,
) -> ValueProvenanceFact {
    let definitely_copy = type_expr_is_copy(return_type, resolved) == Some(true);
    let contains_borrow =
        type_expr_contains_borrow_like(return_type, resolved, &HashMap::new(), &mut HashSet::new());
    if definitely_copy && !contains_borrow {
        ValueProvenanceFact::Independent
    } else {
        fact
    }
}

fn normalize_value_fact(fact: ValueProvenanceFact) -> ValueProvenanceFact {
    match fact {
        ValueProvenanceFact::Aggregate {
            fallback,
            fields,
            elements,
        } => {
            let fallback = fallback.map(|value| Box::new(normalize_value_fact(*value)));
            let fields = fields
                .into_iter()
                .map(|(name, value)| (name, normalize_value_fact(value)))
                .collect::<BTreeMap<_, _>>();
            let elements = elements
                .into_iter()
                .map(|(index, value)| (index, normalize_value_fact(value)))
                .collect::<BTreeMap<_, _>>();
            if fallback
                .as_deref()
                .is_none_or(|value| matches!(value, ValueProvenanceFact::Independent))
                && fields
                    .values()
                    .all(|value| matches!(value, ValueProvenanceFact::Independent))
                && elements
                    .values()
                    .all(|value| matches!(value, ValueProvenanceFact::Independent))
            {
                ValueProvenanceFact::Independent
            } else {
                ValueProvenanceFact::Aggregate {
                    fallback,
                    fields,
                    elements,
                }
            }
        }
        ValueProvenanceFact::Fallible { success, error } => {
            let success = success.map(|value| Box::new(normalize_value_fact(*value)));
            let error = error.map(|value| Box::new(normalize_value_fact(*value)));
            if success
                .as_deref()
                .is_none_or(|value| matches!(value, ValueProvenanceFact::Independent))
                && error
                    .as_deref()
                    .is_none_or(|value| matches!(value, ValueProvenanceFact::Independent))
            {
                ValueProvenanceFact::Independent
            } else {
                ValueProvenanceFact::Fallible { success, error }
            }
        }
        fact @ (ValueProvenanceFact::Independent | ValueProvenanceFact::Origins(_)) => fact,
    }
}

fn value_fact(value: &ValueProvenance) -> ValueProvenanceFact {
    match value {
        ValueProvenance::Independent => ValueProvenanceFact::Independent,
        ValueProvenance::Origins(origins) => {
            ValueProvenanceFact::Origins(origins.iter().map(origin_fact).collect())
        }
        ValueProvenance::Aggregate {
            fallback,
            fields,
            elements,
        } => ValueProvenanceFact::Aggregate {
            fallback: fallback.as_deref().map(value_fact).map(Box::new),
            fields: fields
                .iter()
                .map(|(name, value)| (name.clone(), value_fact(value)))
                .collect(),
            elements: elements
                .iter()
                .map(|(index, value)| (*index, value_fact(value)))
                .collect(),
        },
        ValueProvenance::Fallible { success, error } => ValueProvenanceFact::Fallible {
            success: success.as_deref().map(value_fact).map(Box::new),
            error: error.as_deref().map(value_fact).map(Box::new),
        },
    }
}

fn origin_fact(origin: &StorageOrigin) -> StorageOriginFact {
    match origin {
        StorageOrigin::Static => StorageOriginFact::Static,
        StorageOrigin::CurrentAllocationContext => StorageOriginFact::CurrentAllocationContext,
        StorageOrigin::Input(input) => StorageOriginFact::Input(input.declaration_span()),
        StorageOrigin::Scope { binding, .. } => StorageOriginFact::Scope(*binding),
        StorageOrigin::Region { region, .. } => {
            StorageOriginFact::Region(region.declaration_span())
        }
        StorageOrigin::Unknown => StorageOriginFact::Unknown,
    }
}
