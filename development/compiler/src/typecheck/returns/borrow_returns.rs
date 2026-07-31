use super::terminal::statement_guarantees_return_or_never;
use super::*;

#[derive(Debug, Clone, Default)]
pub(super) struct BorrowReturnEnvironment {
    bindings: HashMap<String, BorrowReturnProvenance>,
}

pub(super) type BorrowReturnSummaries = HashMap<ByteSpan, BorrowReturnProvenance>;

#[derive(Debug, Clone, Default)]
pub(super) struct BorrowReturnFlow {
    value: Option<BorrowReturnProvenance>,
    fallible_error: Option<BorrowReturnProvenance>,
}

impl BorrowReturnFlow {
    pub(super) fn merge_value(&mut self, provenance: Option<BorrowReturnProvenance>) {
        merge_borrow_return_provenance(&mut self.value, provenance);
    }

    pub(super) fn merge_fallible_error(&mut self, provenance: Option<BorrowReturnProvenance>) {
        merge_borrow_return_provenance(&mut self.fallible_error, provenance);
    }

    pub(super) fn into_return_provenance(
        self,
        return_type: &Type,
    ) -> Option<BorrowReturnProvenance> {
        if matches!(return_type, Type::Fallible { .. }) {
            return borrow_return_fallible_provenance(self.value, self.fallible_error);
        }

        self.value
    }
}

impl BorrowReturnEnvironment {
    pub(super) fn get(&self, name: &str) -> Option<&BorrowReturnProvenance> {
        self.bindings.get(name)
    }

    pub(super) fn define_binding(
        &mut self,
        name: String,
        contains_borrow_like: bool,
        provenance: Option<BorrowReturnProvenance>,
    ) {
        if contains_borrow_like {
            if let Some(provenance) = provenance {
                self.bindings.insert(name, provenance);
            } else {
                self.bindings.remove(&name);
            }
        } else {
            self.bindings.remove(&name);
        }
    }

    pub(super) fn join_reachable(&mut self, states: &[BorrowReturnEnvironment]) {
        let mut joined = HashMap::new();
        for state in states {
            for (name, provenance) in &state.bindings {
                joined
                    .entry(name.clone())
                    .and_modify(|existing: &mut BorrowReturnProvenance| existing.merge(provenance))
                    .or_insert_with(|| provenance.clone());
            }
        }
        self.bindings = joined;
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum BorrowReturnProvenance {
    Static,
    InputBorrow {
        sources: BTreeSet<String>,
    },
    Escaping {
        source: String,
    },
    Aggregate {
        fallback: Option<Box<BorrowReturnProvenance>>,
        fields: BTreeMap<String, BorrowReturnProvenance>,
        elements: BTreeMap<usize, BorrowReturnProvenance>,
    },
    Fallible {
        success: Option<Box<BorrowReturnProvenance>>,
        error: Option<Box<BorrowReturnProvenance>>,
    },
}

impl BorrowReturnProvenance {
    pub(super) fn input_borrow(source: String) -> Self {
        Self::InputBorrow {
            sources: BTreeSet::from([source]),
        }
    }

    pub(super) fn escaping(source: String) -> Self {
        Self::Escaping { source }
    }

    pub(super) fn escaping_source(&self) -> Option<&str> {
        match self {
            Self::Escaping { source } => Some(source),
            Self::Aggregate {
                fallback,
                fields,
                elements,
            } => fallback
                .as_deref()
                .and_then(BorrowReturnProvenance::escaping_source)
                .or_else(|| {
                    fields
                        .values()
                        .find_map(BorrowReturnProvenance::escaping_source)
                })
                .or_else(|| {
                    elements
                        .values()
                        .find_map(BorrowReturnProvenance::escaping_source)
                }),
            Self::Fallible { success, error } => success
                .as_deref()
                .and_then(BorrowReturnProvenance::escaping_source)
                .or_else(|| {
                    error
                        .as_deref()
                        .and_then(BorrowReturnProvenance::escaping_source)
                }),
            Self::Static | Self::InputBorrow { .. } => None,
        }
    }

    pub(super) fn success_provenance(&self) -> Option<BorrowReturnProvenance> {
        match self {
            Self::Fallible { success, .. } => success.as_deref().cloned(),
            _ => Some(self.clone()),
        }
    }

    pub(super) fn fallible_error_provenance(&self) -> Option<BorrowReturnProvenance> {
        match self {
            Self::Fallible { error, .. } => error.as_deref().cloned(),
            _ => None,
        }
    }

    pub(super) fn field_provenance(&self, field: &str) -> Option<BorrowReturnProvenance> {
        match self {
            Self::Aggregate {
                fallback, fields, ..
            } => {
                let mut provenance = fallback.as_deref().cloned();
                merge_borrow_return_provenance(&mut provenance, fields.get(field).cloned());
                provenance
            }
            _ => Some(self.clone()),
        }
    }

    pub(super) fn element_provenance(
        &self,
        index: Option<usize>,
    ) -> Option<BorrowReturnProvenance> {
        match self {
            Self::Aggregate {
                fallback, elements, ..
            } => {
                let mut provenance = fallback.as_deref().cloned();
                if let Some(index) = index {
                    merge_borrow_return_provenance(&mut provenance, elements.get(&index).cloned());
                } else {
                    for element_provenance in elements.values() {
                        merge_borrow_return_provenance(
                            &mut provenance,
                            Some(element_provenance.clone()),
                        );
                    }
                }
                provenance
            }
            _ => Some(self.clone()),
        }
    }

    pub(super) fn merge(&mut self, other: &BorrowReturnProvenance) {
        match (&mut *self, other) {
            (Self::Escaping { .. }, _) => {}
            (_, Self::Escaping { source }) => {
                *self = Self::Escaping {
                    source: source.clone(),
                };
            }
            (
                Self::Aggregate {
                    fallback,
                    fields,
                    elements,
                },
                Self::Aggregate {
                    fallback: other_fallback,
                    fields: other_fields,
                    elements: other_elements,
                },
            ) => {
                merge_borrow_return_boxed_provenance(fallback, other_fallback.as_deref().cloned());
                for (field, other_field_provenance) in other_fields {
                    fields
                        .entry(field.clone())
                        .and_modify(|field_provenance| {
                            field_provenance.merge(other_field_provenance)
                        })
                        .or_insert_with(|| other_field_provenance.clone());
                }
                for (index, other_element_provenance) in other_elements {
                    elements
                        .entry(*index)
                        .and_modify(|element_provenance| {
                            element_provenance.merge(other_element_provenance)
                        })
                        .or_insert_with(|| other_element_provenance.clone());
                }
            }
            (
                Self::Fallible { success, error },
                Self::Fallible {
                    success: other_success,
                    error: other_error,
                },
            ) => {
                merge_borrow_return_boxed_provenance(success, other_success.as_deref().cloned());
                merge_borrow_return_boxed_provenance(error, other_error.as_deref().cloned());
            }
            (Self::Fallible { success, .. }, other) => {
                merge_borrow_return_boxed_provenance(success, Some(other.clone()));
            }
            (existing, Self::Fallible { success, error }) => {
                let mut merged_success = success.as_deref().cloned();
                merge_borrow_return_provenance(&mut merged_success, Some(existing.clone()));
                *existing = Self::Fallible {
                    success: merged_success.map(Box::new),
                    error: error.clone(),
                };
            }
            (
                Self::Aggregate {
                    fallback,
                    fields: _,
                    elements: _,
                },
                other,
            ) => {
                merge_borrow_return_boxed_provenance(fallback, Some(other.clone()));
            }
            (
                existing,
                Self::Aggregate {
                    fallback,
                    fields,
                    elements,
                },
            ) => {
                let mut merged_fallback = fallback.as_deref().cloned();
                merge_borrow_return_provenance(&mut merged_fallback, Some(existing.clone()));
                *existing = Self::Aggregate {
                    fallback: merged_fallback.map(Box::new),
                    fields: fields.clone(),
                    elements: elements.clone(),
                };
            }
            (
                Self::InputBorrow { sources },
                Self::InputBorrow {
                    sources: other_sources,
                },
            ) => {
                sources.extend(other_sources.iter().cloned());
            }
            (Self::Static, Self::InputBorrow { sources }) => {
                *self = Self::InputBorrow {
                    sources: sources.clone(),
                };
            }
            (Self::InputBorrow { .. }, Self::Static) | (Self::Static, Self::Static) => {}
        }
    }
}

pub(super) fn borrow_return_summaries(
    summary_sources: &[TypecheckSource<'_>],
) -> BorrowReturnSummaries {
    let mut summaries = BorrowReturnSummaries::new();
    for _ in 0..=borrow_return_callable_count(summary_sources) {
        let next = collect_borrow_return_summaries(summary_sources, &summaries);
        if next == summaries {
            return summaries;
        }
        summaries = next;
    }
    summaries
}

pub(super) fn borrow_return_callable_count(summary_sources: &[TypecheckSource<'_>]) -> usize {
    summary_sources
        .iter()
        .map(|source| {
            source
                .ast
                .items
                .iter()
                .map(item_callable_count)
                .sum::<usize>()
        })
        .sum()
}

pub(super) fn item_callable_count(item: &Item) -> usize {
    match item {
        Item::Function(_) => 1,
        Item::Impl(impl_) => impl_
            .members
            .iter()
            .filter(|member| matches!(member, ImplMember::Method(method) if method.body.is_some()))
            .count(),
        _ => 0,
    }
}

pub(super) fn collect_borrow_return_summaries(
    summary_sources: &[TypecheckSource<'_>],
    previous: &BorrowReturnSummaries,
) -> BorrowReturnSummaries {
    let mut summaries = BorrowReturnSummaries::new();
    for source in summary_sources {
        for item in &source.ast.items {
            match item {
                Item::Function(function) => {
                    let environment = environment_for_function(function, source.resolved);
                    let return_type = type_expr_to_type_in_environment(
                        &function.return_type,
                        source.resolved,
                        &environment,
                    );
                    if type_contains_borrow_like(&return_type, source.resolved) {
                        let provenance = borrow_return_provenance_for_callable_body(
                            &function.body,
                            &return_type,
                            source.resolved,
                            &environment,
                            previous,
                        )
                        .unwrap_or(BorrowReturnProvenance::Static);
                        summaries.insert(function_summary_key(function), provenance);
                    }
                }
                Item::Impl(impl_) => {
                    for member in &impl_.members {
                        let ImplMember::Method(method) = member else {
                            continue;
                        };
                        let Some(body) = &method.body else {
                            continue;
                        };
                        let environment = environment_for_method(method, source.resolved, impl_);
                        let return_type = type_expr_to_type_in_environment(
                            &method.return_type,
                            source.resolved,
                            &environment,
                        );
                        if type_contains_borrow_like(&return_type, source.resolved) {
                            let provenance = borrow_return_provenance_for_callable_body(
                                body,
                                &return_type,
                                source.resolved,
                                &environment,
                                previous,
                            )
                            .unwrap_or(BorrowReturnProvenance::Static);
                            summaries.insert(method.name_span, provenance);
                        }
                    }
                }
                _ => {}
            }
        }
    }
    summaries
}

pub(super) fn function_summary_key(function: &crate::ast::FunctionDecl) -> ByteSpan {
    if function.owner.is_some() {
        function.member_name_span
    } else {
        function.name_span
    }
}

pub(super) fn borrow_return_provenance_for_callable_body(
    block: &Block,
    return_type: &Type,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    summaries: &BorrowReturnSummaries,
) -> Option<BorrowReturnProvenance> {
    if !type_contains_borrow_like(return_type, resolved) {
        return None;
    }

    let mut flow = BorrowReturnFlow::default();
    let mut body_environment = environment.clone();
    let mut body_borrow_provenance = BorrowReturnEnvironment::default();
    collect_return_statement_provenance(
        block,
        return_type,
        resolved,
        &mut body_environment,
        &mut body_borrow_provenance,
        summaries,
        &mut flow,
    );
    collect_block_result_provenance(
        block,
        return_type,
        resolved,
        environment,
        &BorrowReturnEnvironment::default(),
        summaries,
        &mut flow,
    );
    flow.into_return_provenance(return_type)
}

pub(super) fn collect_return_expression_provenance(
    expression: &Expr,
    return_type: &Type,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    borrow_provenance: &BorrowReturnEnvironment,
    summaries: &BorrowReturnSummaries,
    flow: &mut BorrowReturnFlow,
) {
    collect_expression_fallible_propagation_provenance(
        expression,
        return_type,
        resolved,
        environment,
        borrow_provenance,
        summaries,
        flow,
    );

    let actual = expression_type(expression, resolved, environment);
    let provenance = borrow_return_provenance_for_expression(
        expression,
        &actual,
        resolved,
        environment,
        borrow_provenance,
        summaries,
    );
    if expression_is_fallible_failure_for_return_type(
        expression,
        &actual,
        return_type,
        resolved,
        environment,
    ) {
        flow.merge_fallible_error(provenance);
    } else {
        flow.merge_value(provenance);
    }
}

pub(super) fn collect_block_result_provenance(
    block: &Block,
    return_type: &Type,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    borrow_provenance: &BorrowReturnEnvironment,
    summaries: &BorrowReturnSummaries,
    flow: &mut BorrowReturnFlow,
) {
    let Some(result) = &block.result else {
        return;
    };
    let mut result_environment = environment.clone();
    let mut result_borrow_provenance = borrow_provenance.clone();
    apply_borrow_return_statement_effects(
        block,
        resolved,
        &mut result_environment,
        &mut result_borrow_provenance,
        summaries,
    );
    collect_return_expression_provenance(
        result,
        return_type,
        resolved,
        &result_environment,
        &result_borrow_provenance,
        summaries,
        flow,
    );
}

pub(super) fn collect_statement_fallible_propagation_provenance(
    statement: &Stmt,
    return_type: &Type,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    borrow_provenance: &BorrowReturnEnvironment,
    summaries: &BorrowReturnSummaries,
    flow: &mut BorrowReturnFlow,
) {
    match statement {
        Stmt::Import(_)
        | Stmt::FromImport(_)
        | Stmt::Break(_)
        | Stmt::Continue(_)
        | Stmt::Drop(_) => {}
        Stmt::Return(statement) => {
            if let Some(expression) = &statement.expression {
                collect_expression_fallible_propagation_provenance(
                    expression,
                    return_type,
                    resolved,
                    environment,
                    borrow_provenance,
                    summaries,
                    flow,
                );
            }
        }
        Stmt::Binding(statement) => {
            collect_expression_fallible_propagation_provenance(
                &statement.initializer,
                return_type,
                resolved,
                environment,
                borrow_provenance,
                summaries,
                flow,
            );
        }
        Stmt::Assignment(statement) => {
            collect_expression_fallible_propagation_provenance(
                &statement.target,
                return_type,
                resolved,
                environment,
                borrow_provenance,
                summaries,
                flow,
            );
            collect_expression_fallible_propagation_provenance(
                &statement.value,
                return_type,
                resolved,
                environment,
                borrow_provenance,
                summaries,
                flow,
            );
        }
        Stmt::If(statement) => {
            collect_expression_fallible_propagation_provenance(
                &statement.condition,
                return_type,
                resolved,
                environment,
                borrow_provenance,
                summaries,
                flow,
            );
        }
        Stmt::IfIs(statement) => {
            collect_expression_fallible_propagation_provenance(
                &statement.expression,
                return_type,
                resolved,
                environment,
                borrow_provenance,
                summaries,
                flow,
            );
        }
        Stmt::Switch(statement) => {
            collect_expression_fallible_propagation_provenance(
                &statement.expression,
                return_type,
                resolved,
                environment,
                borrow_provenance,
                summaries,
                flow,
            );
        }
        Stmt::While(statement) => {
            collect_expression_fallible_propagation_provenance(
                &statement.condition,
                return_type,
                resolved,
                environment,
                borrow_provenance,
                summaries,
                flow,
            );
        }
        Stmt::ForRange(statement) => {
            collect_expression_fallible_propagation_provenance(
                &statement.start,
                return_type,
                resolved,
                environment,
                borrow_provenance,
                summaries,
                flow,
            );
            collect_expression_fallible_propagation_provenance(
                &statement.end,
                return_type,
                resolved,
                environment,
                borrow_provenance,
                summaries,
                flow,
            );
        }
        Stmt::Loop(_) => {}
        Stmt::Expression(statement) => {
            collect_expression_fallible_propagation_provenance(
                &statement.expression,
                return_type,
                resolved,
                environment,
                borrow_provenance,
                summaries,
                flow,
            );
        }
    }
}

pub(super) fn collect_expression_fallible_propagation_provenance(
    expression: &Expr,
    return_type: &Type,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    borrow_provenance: &BorrowReturnEnvironment,
    summaries: &BorrowReturnSummaries,
    flow: &mut BorrowReturnFlow,
) {
    match expression {
        Expr::Propagate(propagation) => {
            if propagated_fallible_error_can_escape(
                &propagation.expression,
                return_type,
                resolved,
                environment,
            ) {
                flow.merge_fallible_error(borrow_return_fallible_error_provenance_for_expression(
                    &propagation.expression,
                    resolved,
                    environment,
                    borrow_provenance,
                    summaries,
                ));
            }
            collect_expression_fallible_propagation_provenance(
                &propagation.expression,
                return_type,
                resolved,
                environment,
                borrow_provenance,
                summaries,
                flow,
            );
        }
        Expr::Catch(catch) => {
            collect_expression_fallible_propagation_provenance(
                &catch.expression,
                return_type,
                resolved,
                environment,
                borrow_provenance,
                summaries,
                flow,
            );
            collect_block_fallible_propagation_provenance(
                &catch.catch_block,
                return_type,
                resolved,
                environment,
                borrow_provenance,
                summaries,
                flow,
            );
        }
        Expr::Force(force) => collect_expression_fallible_propagation_provenance(
            &force.expression,
            return_type,
            resolved,
            environment,
            borrow_provenance,
            summaries,
            flow,
        ),
        Expr::Borrow(borrow) => collect_expression_fallible_propagation_provenance(
            &borrow.expression,
            return_type,
            resolved,
            environment,
            borrow_provenance,
            summaries,
            flow,
        ),
        Expr::Unary(unary) => collect_expression_fallible_propagation_provenance(
            &unary.operand,
            return_type,
            resolved,
            environment,
            borrow_provenance,
            summaries,
            flow,
        ),
        Expr::Binary(binary) => {
            collect_expression_fallible_propagation_provenance(
                &binary.left,
                return_type,
                resolved,
                environment,
                borrow_provenance,
                summaries,
                flow,
            );
            collect_expression_fallible_propagation_provenance(
                &binary.right,
                return_type,
                resolved,
                environment,
                borrow_provenance,
                summaries,
                flow,
            );
        }
        Expr::TypeConversion(conversion) => collect_expression_fallible_propagation_provenance(
            &conversion.expression,
            return_type,
            resolved,
            environment,
            borrow_provenance,
            summaries,
            flow,
        ),
        Expr::Call(call) => {
            collect_expression_fallible_propagation_provenance(
                &call.callee,
                return_type,
                resolved,
                environment,
                borrow_provenance,
                summaries,
                flow,
            );
            for argument in &call.arguments {
                collect_expression_fallible_propagation_provenance(
                    argument,
                    return_type,
                    resolved,
                    environment,
                    borrow_provenance,
                    summaries,
                    flow,
                );
            }
        }
        Expr::Member(member) => collect_expression_fallible_propagation_provenance(
            &member.object,
            return_type,
            resolved,
            environment,
            borrow_provenance,
            summaries,
            flow,
        ),
        Expr::Index(index) => {
            collect_expression_fallible_propagation_provenance(
                &index.object,
                return_type,
                resolved,
                environment,
                borrow_provenance,
                summaries,
                flow,
            );
            collect_expression_fallible_propagation_provenance(
                &index.index,
                return_type,
                resolved,
                environment,
                borrow_provenance,
                summaries,
                flow,
            );
        }
        Expr::Group(group) => collect_expression_fallible_propagation_provenance(
            &group.expression,
            return_type,
            resolved,
            environment,
            borrow_provenance,
            summaries,
            flow,
        ),
        Expr::Otherwise(otherwise) => {
            collect_expression_fallible_propagation_provenance(
                &otherwise.value,
                return_type,
                resolved,
                environment,
                borrow_provenance,
                summaries,
                flow,
            );
            collect_block_fallible_propagation_provenance(
                &otherwise.fallback,
                return_type,
                resolved,
                environment,
                borrow_provenance,
                summaries,
                flow,
            );
        }
        Expr::If(expression) => {
            collect_expression_fallible_propagation_provenance(
                &expression.condition,
                return_type,
                resolved,
                environment,
                borrow_provenance,
                summaries,
                flow,
            );
            collect_block_fallible_propagation_provenance(
                &expression.then_block,
                return_type,
                resolved,
                environment,
                borrow_provenance,
                summaries,
                flow,
            );
            if let Some(else_block) = &expression.else_block {
                collect_block_fallible_propagation_provenance(
                    else_block,
                    return_type,
                    resolved,
                    environment,
                    borrow_provenance,
                    summaries,
                    flow,
                );
            }
        }
        Expr::IfIs(expression) => {
            collect_expression_fallible_propagation_provenance(
                &expression.expression,
                return_type,
                resolved,
                environment,
                borrow_provenance,
                summaries,
                flow,
            );
            let then_environment = environment_for_if_is_binding(expression, resolved, environment);
            let mut then_borrow_provenance = borrow_provenance.clone();
            define_if_is_payload_borrow_return_binding(
                expression,
                resolved,
                environment,
                &then_environment,
                &mut then_borrow_provenance,
                summaries,
            );
            collect_block_fallible_propagation_provenance(
                &expression.then_block,
                return_type,
                resolved,
                &then_environment,
                &then_borrow_provenance,
                summaries,
                flow,
            );
            if let Some(else_block) = &expression.else_block {
                collect_block_fallible_propagation_provenance(
                    else_block,
                    return_type,
                    resolved,
                    environment,
                    borrow_provenance,
                    summaries,
                    flow,
                );
            }
        }
        Expr::Match(expression) => {
            collect_expression_fallible_propagation_provenance(
                &expression.expression,
                return_type,
                resolved,
                environment,
                borrow_provenance,
                summaries,
                flow,
            );
            for arm in &expression.arms {
                let arm_environment =
                    environment_for_switch_arm(arm, &expression.expression, resolved, environment);
                let mut arm_borrow_provenance = borrow_provenance.clone();
                define_switch_arm_payload_borrow_return_binding(
                    arm,
                    &expression.expression,
                    resolved,
                    environment,
                    &arm_environment,
                    &mut arm_borrow_provenance,
                    summaries,
                );
                collect_block_fallible_propagation_provenance(
                    &arm.body,
                    return_type,
                    resolved,
                    &arm_environment,
                    &arm_borrow_provenance,
                    summaries,
                    flow,
                );
            }
            if let Some(wildcard_arm) = &expression.wildcard_arm {
                collect_block_fallible_propagation_provenance(
                    &wildcard_arm.body,
                    return_type,
                    resolved,
                    environment,
                    borrow_provenance,
                    summaries,
                    flow,
                );
            }
        }
        Expr::ArrayLiteral(literal) => {
            for element in &literal.elements {
                collect_expression_fallible_propagation_provenance(
                    element,
                    return_type,
                    resolved,
                    environment,
                    borrow_provenance,
                    summaries,
                    flow,
                );
            }
        }
        Expr::StructLiteral(literal) => {
            for field in &literal.fields {
                collect_expression_fallible_propagation_provenance(
                    &field.value,
                    return_type,
                    resolved,
                    environment,
                    borrow_provenance,
                    summaries,
                    flow,
                );
            }
        }
        Expr::InterpolatedString(interpolated) => {
            for part in &interpolated.parts {
                if let InterpolatedStringPart::Expression(part) = part {
                    collect_expression_fallible_propagation_provenance(
                        &part.expression,
                        return_type,
                        resolved,
                        environment,
                        borrow_provenance,
                        summaries,
                        flow,
                    );
                }
            }
        }
        Expr::Identifier(_)
        | Expr::IntegerLiteral(_)
        | Expr::ByteLiteral(_)
        | Expr::StringLiteral(_)
        | Expr::BoolLiteral(_)
        | Expr::NoneLiteral(_) => {}
    }
}

pub(super) fn collect_block_fallible_propagation_provenance(
    block: &Block,
    return_type: &Type,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    borrow_provenance: &BorrowReturnEnvironment,
    summaries: &BorrowReturnSummaries,
    flow: &mut BorrowReturnFlow,
) {
    let mut block_environment = environment.clone();
    let mut block_borrow_provenance = borrow_provenance.clone();
    for statement in &block.statements {
        collect_statement_fallible_propagation_provenance(
            statement,
            return_type,
            resolved,
            &block_environment,
            &block_borrow_provenance,
            summaries,
            flow,
        );
        apply_borrow_return_statement_effect(
            statement,
            resolved,
            &mut block_environment,
            &mut block_borrow_provenance,
            summaries,
        );
        if statement_guarantees_return_or_never(statement, resolved, &block_environment) {
            return;
        }
    }
    if let Some(result) = &block.result {
        collect_expression_fallible_propagation_provenance(
            result,
            return_type,
            resolved,
            &block_environment,
            &block_borrow_provenance,
            summaries,
            flow,
        );
    }
}

pub(super) fn collect_return_statement_provenance(
    block: &Block,
    return_type: &Type,
    resolved: &ResolveOutput,
    environment: &mut TypeEnvironment,
    borrow_provenance: &mut BorrowReturnEnvironment,
    summaries: &BorrowReturnSummaries,
    flow: &mut BorrowReturnFlow,
) {
    for statement in &block.statements {
        collect_statement_fallible_propagation_provenance(
            statement,
            return_type,
            resolved,
            environment,
            borrow_provenance,
            summaries,
            flow,
        );
        match statement {
            Stmt::Return(statement) => {
                if let Some(expression) = &statement.expression {
                    collect_return_expression_provenance(
                        expression,
                        return_type,
                        resolved,
                        environment,
                        borrow_provenance,
                        summaries,
                        flow,
                    );
                }
            }
            Stmt::If(if_statement) => {
                let mut then_environment = environment.clone();
                let mut then_borrow_provenance = borrow_provenance.clone();
                collect_return_statement_provenance(
                    &if_statement.then_block,
                    return_type,
                    resolved,
                    &mut then_environment,
                    &mut then_borrow_provenance,
                    summaries,
                    flow,
                );
                if let Some(else_block) = &if_statement.else_block {
                    let mut else_environment = environment.clone();
                    let mut else_borrow_provenance = borrow_provenance.clone();
                    collect_return_statement_provenance(
                        else_block,
                        return_type,
                        resolved,
                        &mut else_environment,
                        &mut else_borrow_provenance,
                        summaries,
                        flow,
                    );
                }
                apply_borrow_return_statement_effect(
                    statement,
                    resolved,
                    environment,
                    borrow_provenance,
                    summaries,
                );
            }
            Stmt::IfIs(if_is_statement) => {
                let mut then_environment =
                    environment_for_if_is_binding(if_is_statement, resolved, environment);
                let mut then_borrow_provenance = borrow_provenance.clone();
                define_if_is_payload_borrow_return_binding(
                    if_is_statement,
                    resolved,
                    environment,
                    &then_environment,
                    &mut then_borrow_provenance,
                    summaries,
                );
                collect_return_statement_provenance(
                    &if_is_statement.then_block,
                    return_type,
                    resolved,
                    &mut then_environment,
                    &mut then_borrow_provenance,
                    summaries,
                    flow,
                );
                if let Some(else_block) = &if_is_statement.else_block {
                    let mut else_environment = environment.clone();
                    let mut else_borrow_provenance = borrow_provenance.clone();
                    collect_return_statement_provenance(
                        else_block,
                        return_type,
                        resolved,
                        &mut else_environment,
                        &mut else_borrow_provenance,
                        summaries,
                        flow,
                    );
                }
                apply_borrow_return_statement_effect(
                    statement,
                    resolved,
                    environment,
                    borrow_provenance,
                    summaries,
                );
            }
            Stmt::Switch(switch_statement) => {
                for arm in &switch_statement.arms {
                    let mut arm_environment = environment_for_switch_arm(
                        arm,
                        &switch_statement.expression,
                        resolved,
                        environment,
                    );
                    let mut arm_borrow_provenance = borrow_provenance.clone();
                    define_switch_arm_payload_borrow_return_binding(
                        arm,
                        &switch_statement.expression,
                        resolved,
                        environment,
                        &arm_environment,
                        &mut arm_borrow_provenance,
                        summaries,
                    );
                    collect_return_statement_provenance(
                        &arm.body,
                        return_type,
                        resolved,
                        &mut arm_environment,
                        &mut arm_borrow_provenance,
                        summaries,
                        flow,
                    );
                }
                if let Some(wildcard_arm) = &switch_statement.wildcard_arm {
                    let mut wildcard_environment = environment.clone();
                    let mut wildcard_borrow_provenance = borrow_provenance.clone();
                    collect_return_statement_provenance(
                        &wildcard_arm.body,
                        return_type,
                        resolved,
                        &mut wildcard_environment,
                        &mut wildcard_borrow_provenance,
                        summaries,
                        flow,
                    );
                }
                apply_borrow_return_statement_effect(
                    statement,
                    resolved,
                    environment,
                    borrow_provenance,
                    summaries,
                );
            }
            Stmt::While(statement) => {
                let mut body_environment = environment.clone();
                let mut body_borrow_provenance = borrow_provenance.clone();
                collect_return_statement_provenance(
                    &statement.body,
                    return_type,
                    resolved,
                    &mut body_environment,
                    &mut body_borrow_provenance,
                    summaries,
                    flow,
                );
            }
            Stmt::ForRange(statement) => {
                let mut body_environment =
                    environment_for_for_range_binding(statement, resolved, environment);
                let mut body_borrow_provenance = borrow_provenance.clone();
                collect_return_statement_provenance(
                    &statement.body,
                    return_type,
                    resolved,
                    &mut body_environment,
                    &mut body_borrow_provenance,
                    summaries,
                    flow,
                );
            }
            Stmt::Loop(statement) => {
                let mut body_environment = environment.clone();
                let mut body_borrow_provenance = borrow_provenance.clone();
                collect_return_statement_provenance(
                    &statement.body,
                    return_type,
                    resolved,
                    &mut body_environment,
                    &mut body_borrow_provenance,
                    summaries,
                    flow,
                );
            }
            _ => {
                apply_borrow_return_statement_effect(
                    statement,
                    resolved,
                    environment,
                    borrow_provenance,
                    summaries,
                );
            }
        }
        if statement_guarantees_return_or_never(statement, resolved, environment) {
            return;
        }
    }
}

pub(super) fn borrow_return_provenance_for_expression(
    expression: &Expr,
    ty: &Type,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    borrow_provenance: &BorrowReturnEnvironment,
    summaries: &BorrowReturnSummaries,
) -> Option<BorrowReturnProvenance> {
    if !type_contains_borrow_like(ty, resolved) {
        return None;
    }

    match unwrap_group(expression) {
        Expr::Borrow(_) => borrow_return_provenance_for_direct_borrow(expression, resolved),
        Expr::Identifier(identifier) => borrow_return_provenance_for_identifier(
            identifier,
            resolved,
            environment,
            borrow_provenance,
        ),
        Expr::Force(expression) => borrow_return_success_provenance_for_expression(
            &expression.expression,
            resolved,
            environment,
            borrow_provenance,
            summaries,
        ),
        Expr::Propagate(expression) => borrow_return_success_provenance_for_expression(
            &expression.expression,
            resolved,
            environment,
            borrow_provenance,
            summaries,
        ),
        Expr::Catch(expression) => borrow_return_success_provenance_for_expression(
            &expression.expression,
            resolved,
            environment,
            borrow_provenance,
            summaries,
        ),
        Expr::StringLiteral(_) => Some(BorrowReturnProvenance::Static),
        Expr::StructLiteral(literal) => {
            let mut fields = BTreeMap::new();
            for field in &literal.fields {
                let field_type = expression_type(&field.value, resolved, environment);
                if let Some(field_provenance) = borrow_return_provenance_for_expression(
                    &field.value,
                    &field_type,
                    resolved,
                    environment,
                    borrow_provenance,
                    summaries,
                ) {
                    fields.insert(field.name.clone(), field_provenance);
                }
            }
            (!fields.is_empty()).then_some(BorrowReturnProvenance::Aggregate {
                fallback: None,
                fields,
                elements: BTreeMap::new(),
            })
        }
        Expr::Member(member) => borrow_return_provenance_for_member(
            member,
            resolved,
            environment,
            borrow_provenance,
            summaries,
        ),
        Expr::Index(index) => borrow_return_provenance_for_index(
            index,
            resolved,
            environment,
            borrow_provenance,
            summaries,
        ),
        Expr::ArrayLiteral(literal) => {
            let mut elements = BTreeMap::new();
            for (index, element) in literal.elements.iter().enumerate() {
                let element_type = expression_type(element, resolved, environment);
                if let Some(element_provenance) = borrow_return_provenance_for_expression(
                    element,
                    &element_type,
                    resolved,
                    environment,
                    borrow_provenance,
                    summaries,
                ) {
                    elements.insert(index, element_provenance);
                }
            }
            (!elements.is_empty()).then_some(BorrowReturnProvenance::Aggregate {
                fallback: None,
                fields: BTreeMap::new(),
                elements,
            })
        }
        Expr::Call(call) if is_enum_variant_call(call, resolved) => {
            let mut provenance = None;
            for argument in &call.arguments {
                let argument_type = expression_type(argument, resolved, environment);
                merge_borrow_return_provenance(
                    &mut provenance,
                    borrow_return_provenance_for_expression(
                        argument,
                        &argument_type,
                        resolved,
                        environment,
                        borrow_provenance,
                        summaries,
                    ),
                );
            }
            provenance
        }
        Expr::Call(call) => borrow_return_provenance_for_call(
            call,
            resolved,
            environment,
            borrow_provenance,
            summaries,
        ),
        Expr::Otherwise(expression) => {
            let mut provenance = borrow_return_provenance_for_expression(
                &expression.value,
                &expression_type(&expression.value, resolved, environment),
                resolved,
                environment,
                borrow_provenance,
                summaries,
            );
            merge_borrow_return_provenance(
                &mut provenance,
                borrow_return_provenance_for_block_result(
                    &expression.fallback,
                    resolved,
                    environment,
                    borrow_provenance,
                    summaries,
                ),
            );
            provenance
        }
        Expr::If(expression) => {
            let Some(else_block) = &expression.else_block else {
                return None;
            };
            let mut provenance = borrow_return_provenance_for_block_result(
                &expression.then_block,
                resolved,
                environment,
                borrow_provenance,
                summaries,
            );
            merge_borrow_return_provenance(
                &mut provenance,
                borrow_return_provenance_for_block_result(
                    else_block,
                    resolved,
                    environment,
                    borrow_provenance,
                    summaries,
                ),
            );
            provenance
        }
        Expr::IfIs(expression) => {
            let Some(else_block) = &expression.else_block else {
                return None;
            };
            let then_environment = environment_for_if_is_binding(expression, resolved, environment);
            let mut then_borrow_provenance = borrow_provenance.clone();
            define_if_is_payload_borrow_return_binding(
                expression,
                resolved,
                environment,
                &then_environment,
                &mut then_borrow_provenance,
                summaries,
            );
            let mut provenance = borrow_return_provenance_for_block_result(
                &expression.then_block,
                resolved,
                &then_environment,
                &then_borrow_provenance,
                summaries,
            );
            merge_borrow_return_provenance(
                &mut provenance,
                borrow_return_provenance_for_block_result(
                    else_block,
                    resolved,
                    environment,
                    borrow_provenance,
                    summaries,
                ),
            );
            provenance
        }
        Expr::Match(expression) => {
            let mut provenance = None;
            for arm in &expression.arms {
                let arm_environment =
                    environment_for_switch_arm(arm, &expression.expression, resolved, environment);
                let mut arm_borrow_provenance = borrow_provenance.clone();
                define_switch_arm_payload_borrow_return_binding(
                    arm,
                    &expression.expression,
                    resolved,
                    environment,
                    &arm_environment,
                    &mut arm_borrow_provenance,
                    summaries,
                );
                merge_borrow_return_provenance(
                    &mut provenance,
                    borrow_return_provenance_for_block_result(
                        &arm.body,
                        resolved,
                        &arm_environment,
                        &arm_borrow_provenance,
                        summaries,
                    ),
                );
            }
            if let Some(wildcard_arm) = &expression.wildcard_arm {
                merge_borrow_return_provenance(
                    &mut provenance,
                    borrow_return_provenance_for_block_result(
                        &wildcard_arm.body,
                        resolved,
                        environment,
                        borrow_provenance,
                        summaries,
                    ),
                );
            }
            provenance
        }
        _ => None,
    }
}

pub(super) fn borrow_return_success_provenance_for_expression(
    expression: &Expr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    borrow_provenance: &BorrowReturnEnvironment,
    summaries: &BorrowReturnSummaries,
) -> Option<BorrowReturnProvenance> {
    let expression_type = expression_type(expression, resolved, environment);
    borrow_return_provenance_for_expression(
        expression,
        &expression_type,
        resolved,
        environment,
        borrow_provenance,
        summaries,
    )
    .and_then(|provenance| provenance.success_provenance())
}

pub(super) fn borrow_return_fallible_error_provenance_for_expression(
    expression: &Expr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    borrow_provenance: &BorrowReturnEnvironment,
    summaries: &BorrowReturnSummaries,
) -> Option<BorrowReturnProvenance> {
    let expression_type = expression_type(expression, resolved, environment);
    if !matches!(expression_type, Type::Fallible { .. }) {
        return None;
    }
    borrow_return_provenance_for_expression(
        expression,
        &expression_type,
        resolved,
        environment,
        borrow_provenance,
        summaries,
    )
    .and_then(|provenance| provenance.fallible_error_provenance())
}

pub(super) fn borrow_return_provenance_for_member(
    member: &crate::ast::MemberExpr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    borrow_provenance: &BorrowReturnEnvironment,
    summaries: &BorrowReturnSummaries,
) -> Option<BorrowReturnProvenance> {
    let object_type = expression_type(&member.object, resolved, environment);
    borrow_return_provenance_for_expression(
        &member.object,
        &object_type,
        resolved,
        environment,
        borrow_provenance,
        summaries,
    )
    .and_then(|provenance| provenance.field_provenance(&member.member))
}

pub(super) fn borrow_return_provenance_for_index(
    index: &crate::ast::IndexExpr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    borrow_provenance: &BorrowReturnEnvironment,
    summaries: &BorrowReturnSummaries,
) -> Option<BorrowReturnProvenance> {
    let object_type = expression_type(&index.object, resolved, environment);
    borrow_return_provenance_for_expression(
        &index.object,
        &object_type,
        resolved,
        environment,
        borrow_provenance,
        summaries,
    )
    .and_then(|provenance| provenance.element_provenance(index_literal_value(&index.index)))
}

pub(super) fn index_literal_value(expression: &Expr) -> Option<usize> {
    integer_literal_expr_value(expression).and_then(|value| usize::try_from(value).ok())
}

pub(super) fn borrow_return_provenance_for_call(
    call: &crate::ast::CallExpr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    borrow_provenance: &BorrowReturnEnvironment,
    summaries: &BorrowReturnSummaries,
) -> Option<BorrowReturnProvenance> {
    let signature = resolved_call_signature(resolved, call, environment)?;
    let return_type = call_return_type(call, &signature, resolved, environment);
    if let Some(declaration_span) = signature.declaration_span
        && let Some(summary) = summaries.get(&declaration_span)
    {
        return borrow_return_provenance_for_call_summary(
            summary,
            call,
            &signature,
            resolved,
            environment,
            borrow_provenance,
            summaries,
        );
    }

    let mut provenance = None;
    if let Some((_, method)) = resolved_method_for_call(resolved, call, environment)
        && method_receiver_is_borrow(method)
        && let Some(member) = method_member_for_call(call)
        && let Some(receiver_provenance) = borrow_return_provenance_for_borrowed_input(
            &member.object,
            resolved,
            environment,
            borrow_provenance,
            summaries,
        )
    {
        merge_borrow_return_provenance(&mut provenance, Some(receiver_provenance));
    }

    for (argument, parameter) in call.arguments.iter().zip(&signature.signature.parameters) {
        let argument_type = expression_type(argument, resolved, environment);
        if !type_contains_borrow_like(&argument_type, resolved)
            && !type_expr_contains_borrow_like(
                &parameter.ty,
                resolved,
                &HashMap::new(),
                &mut HashSet::new(),
            )
        {
            continue;
        }

        merge_borrow_return_provenance(
            &mut provenance,
            borrow_return_provenance_for_borrowed_input(
                argument,
                resolved,
                environment,
                borrow_provenance,
                summaries,
            ),
        );
    }

    match return_type {
        Type::Fallible { success, error } => {
            let success_provenance = type_contains_borrow_like(&success, resolved)
                .then(|| provenance.clone())
                .flatten();
            let error_provenance = type_contains_borrow_like(&error, resolved)
                .then_some(provenance)
                .flatten();
            borrow_return_fallible_provenance(success_provenance, error_provenance)
        }
        _ => provenance,
    }
}

pub(super) fn borrow_return_provenance_for_call_summary(
    summary: &BorrowReturnProvenance,
    call: &crate::ast::CallExpr,
    signature: &crate::typecheck::calls::CheckedCallSignature<'_>,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    borrow_provenance: &BorrowReturnEnvironment,
    summaries: &BorrowReturnSummaries,
) -> Option<BorrowReturnProvenance> {
    match summary {
        BorrowReturnProvenance::Static => Some(BorrowReturnProvenance::Static),
        BorrowReturnProvenance::Escaping { .. } => None,
        BorrowReturnProvenance::Fallible { success, error } => {
            let mapped_success = success.as_deref().and_then(|provenance| {
                borrow_return_provenance_for_call_summary(
                    provenance,
                    call,
                    signature,
                    resolved,
                    environment,
                    borrow_provenance,
                    summaries,
                )
            });
            let mapped_error = error.as_deref().and_then(|provenance| {
                borrow_return_provenance_for_call_summary(
                    provenance,
                    call,
                    signature,
                    resolved,
                    environment,
                    borrow_provenance,
                    summaries,
                )
            });
            borrow_return_fallible_provenance(mapped_success, mapped_error)
        }
        BorrowReturnProvenance::Aggregate {
            fallback,
            fields,
            elements,
        } => {
            let mapped_fallback = fallback.as_deref().and_then(|provenance| {
                borrow_return_provenance_for_call_summary(
                    provenance,
                    call,
                    signature,
                    resolved,
                    environment,
                    borrow_provenance,
                    summaries,
                )
            });
            let mut mapped_fields = BTreeMap::new();
            for (field, field_provenance) in fields {
                if let Some(mapped_field) = borrow_return_provenance_for_call_summary(
                    field_provenance,
                    call,
                    signature,
                    resolved,
                    environment,
                    borrow_provenance,
                    summaries,
                ) {
                    mapped_fields.insert(field.clone(), mapped_field);
                }
            }
            let mut mapped_elements = BTreeMap::new();
            for (index, element_provenance) in elements {
                if let Some(mapped_element) = borrow_return_provenance_for_call_summary(
                    element_provenance,
                    call,
                    signature,
                    resolved,
                    environment,
                    borrow_provenance,
                    summaries,
                ) {
                    mapped_elements.insert(*index, mapped_element);
                }
            }
            if mapped_fallback.is_none() && mapped_fields.is_empty() && mapped_elements.is_empty() {
                None
            } else {
                Some(BorrowReturnProvenance::Aggregate {
                    fallback: mapped_fallback.map(Box::new),
                    fields: mapped_fields,
                    elements: mapped_elements,
                })
            }
        }
        BorrowReturnProvenance::InputBorrow { sources } => {
            let mut provenance = None;
            for source in sources {
                merge_borrow_return_provenance(
                    &mut provenance,
                    borrow_return_provenance_for_call_input(
                        source,
                        call,
                        signature,
                        resolved,
                        environment,
                        borrow_provenance,
                        summaries,
                    ),
                );
            }
            provenance
        }
    }
}

pub(super) fn borrow_return_provenance_for_call_input(
    source: &str,
    call: &crate::ast::CallExpr,
    signature: &crate::typecheck::calls::CheckedCallSignature<'_>,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    borrow_provenance: &BorrowReturnEnvironment,
    summaries: &BorrowReturnSummaries,
) -> Option<BorrowReturnProvenance> {
    if signature.kind == crate::typecheck::calls::CheckedCallKind::Method
        && let Some((_, method)) = resolved_method_for_call(resolved, call, environment)
        && method.receiver.name == source
        && let Some(member) = method_member_for_call(call)
    {
        return borrow_return_provenance_for_borrowed_input(
            &member.object,
            resolved,
            environment,
            borrow_provenance,
            summaries,
        );
    }

    for (index, parameter) in signature.signature.parameters.iter().enumerate() {
        if parameter.name == source {
            return call.arguments.get(index).and_then(|argument| {
                borrow_return_provenance_for_borrowed_input(
                    argument,
                    resolved,
                    environment,
                    borrow_provenance,
                    summaries,
                )
            });
        }
    }

    None
}

pub(super) fn borrow_return_fallible_provenance(
    success: Option<BorrowReturnProvenance>,
    error: Option<BorrowReturnProvenance>,
) -> Option<BorrowReturnProvenance> {
    if success.is_none() && error.is_none() {
        return None;
    }

    Some(BorrowReturnProvenance::Fallible {
        success: success.map(Box::new),
        error: error.map(Box::new),
    })
}

pub(super) fn merge_borrow_return_provenance(
    provenance: &mut Option<BorrowReturnProvenance>,
    next: Option<BorrowReturnProvenance>,
) {
    let Some(next) = next else {
        return;
    };
    if let Some(existing) = provenance {
        existing.merge(&next);
    } else {
        *provenance = Some(next);
    }
}

pub(super) fn merge_borrow_return_boxed_provenance(
    provenance: &mut Option<Box<BorrowReturnProvenance>>,
    next: Option<BorrowReturnProvenance>,
) {
    let mut unboxed = provenance.take().map(|provenance| *provenance);
    merge_borrow_return_provenance(&mut unboxed, next);
    *provenance = unboxed.map(Box::new);
}

pub(super) fn borrow_return_provenance_for_block_result(
    block: &crate::ast::Block,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    borrow_provenance: &BorrowReturnEnvironment,
    summaries: &BorrowReturnSummaries,
) -> Option<BorrowReturnProvenance> {
    let Some(result) = &block.result else {
        return None;
    };
    let mut result_environment = environment.clone();
    let mut result_borrow_provenance = borrow_provenance.clone();
    apply_borrow_return_statement_effects(
        block,
        resolved,
        &mut result_environment,
        &mut result_borrow_provenance,
        summaries,
    );
    let result_type = expression_type(result, resolved, &result_environment);
    borrow_return_provenance_for_expression(
        result,
        &result_type,
        resolved,
        &result_environment,
        &result_borrow_provenance,
        summaries,
    )
}

pub(super) fn apply_borrow_return_statement_effects(
    block: &crate::ast::Block,
    resolved: &ResolveOutput,
    environment: &mut TypeEnvironment,
    borrow_provenance: &mut BorrowReturnEnvironment,
    summaries: &BorrowReturnSummaries,
) {
    for statement in &block.statements {
        apply_borrow_return_statement_effect(
            statement,
            resolved,
            environment,
            borrow_provenance,
            summaries,
        );
    }
}

pub(super) fn apply_borrow_return_statement_effect(
    statement: &Stmt,
    resolved: &ResolveOutput,
    environment: &mut TypeEnvironment,
    borrow_provenance: &mut BorrowReturnEnvironment,
    summaries: &BorrowReturnSummaries,
) {
    match statement {
        Stmt::Binding(statement) => {
            let initializer_type = expression_type(&statement.initializer, resolved, environment);
            let binding_type =
                continuing_binding_type(statement, initializer_type, resolved, environment);
            let provenance = borrow_return_provenance_for_expression(
                &statement.initializer,
                &binding_type,
                resolved,
                environment,
                borrow_provenance,
                summaries,
            );
            environment.define_binding(
                statement.name.clone(),
                binding_type.clone(),
                binding_kind_is_mutable(statement.kind),
            );
            borrow_provenance.define_binding(
                statement.name.clone(),
                type_contains_borrow_like(&binding_type, resolved),
                provenance,
            );
        }
        Stmt::Assignment(statement) => {
            if let Some(identifier) = whole_identifier(&statement.target)
                && let Some(target_type) = environment.get(&identifier.name)
            {
                let provenance = borrow_return_provenance_for_expression(
                    &statement.value,
                    target_type,
                    resolved,
                    environment,
                    borrow_provenance,
                    summaries,
                );
                borrow_provenance.define_binding(
                    identifier.name.clone(),
                    type_contains_borrow_like(target_type, resolved),
                    provenance,
                );
            }
        }
        Stmt::If(statement) => {
            let mut then_environment = environment.clone();
            let mut then_borrow_provenance = borrow_provenance.clone();
            apply_borrow_return_statement_effects(
                &statement.then_block,
                resolved,
                &mut then_environment,
                &mut then_borrow_provenance,
                summaries,
            );
            let mut incoming = vec![then_borrow_provenance];
            if let Some(else_block) = &statement.else_block {
                let mut else_environment = environment.clone();
                let mut else_borrow_provenance = borrow_provenance.clone();
                apply_borrow_return_statement_effects(
                    else_block,
                    resolved,
                    &mut else_environment,
                    &mut else_borrow_provenance,
                    summaries,
                );
                incoming.push(else_borrow_provenance);
            } else {
                incoming.push(borrow_provenance.clone());
            }
            borrow_provenance.join_reachable(&incoming);
        }
        Stmt::IfIs(statement) => {
            let mut then_environment =
                environment_for_if_is_binding(statement, resolved, environment);
            let mut then_borrow_provenance = borrow_provenance.clone();
            define_if_is_payload_borrow_return_binding(
                statement,
                resolved,
                environment,
                &then_environment,
                &mut then_borrow_provenance,
                summaries,
            );
            apply_borrow_return_statement_effects(
                &statement.then_block,
                resolved,
                &mut then_environment,
                &mut then_borrow_provenance,
                summaries,
            );
            let mut incoming = vec![then_borrow_provenance];
            if let Some(else_block) = &statement.else_block {
                let mut else_environment = environment.clone();
                let mut else_borrow_provenance = borrow_provenance.clone();
                apply_borrow_return_statement_effects(
                    else_block,
                    resolved,
                    &mut else_environment,
                    &mut else_borrow_provenance,
                    summaries,
                );
                incoming.push(else_borrow_provenance);
            } else {
                incoming.push(borrow_provenance.clone());
            }
            borrow_provenance.join_reachable(&incoming);
        }
        Stmt::Switch(statement) => {
            let mut incoming = Vec::new();
            for arm in &statement.arms {
                let mut arm_environment =
                    environment_for_switch_arm(arm, &statement.expression, resolved, environment);
                let mut arm_borrow_provenance = borrow_provenance.clone();
                define_switch_arm_payload_borrow_return_binding(
                    arm,
                    &statement.expression,
                    resolved,
                    environment,
                    &arm_environment,
                    &mut arm_borrow_provenance,
                    summaries,
                );
                apply_borrow_return_statement_effects(
                    &arm.body,
                    resolved,
                    &mut arm_environment,
                    &mut arm_borrow_provenance,
                    summaries,
                );
                incoming.push(arm_borrow_provenance);
            }
            if let Some(wildcard_arm) = &statement.wildcard_arm {
                let mut wildcard_environment = environment.clone();
                let mut wildcard_borrow_provenance = borrow_provenance.clone();
                apply_borrow_return_statement_effects(
                    &wildcard_arm.body,
                    resolved,
                    &mut wildcard_environment,
                    &mut wildcard_borrow_provenance,
                    summaries,
                );
                incoming.push(wildcard_borrow_provenance);
            } else {
                incoming.push(borrow_provenance.clone());
            }
            borrow_provenance.join_reachable(&incoming);
        }
        _ => {}
    }
}

pub(super) fn define_if_is_payload_borrow_return_binding(
    statement: &IfIsStmt,
    resolved: &ResolveOutput,
    source_environment: &TypeEnvironment,
    payload_environment: &TypeEnvironment,
    borrow_provenance: &mut BorrowReturnEnvironment,
    summaries: &BorrowReturnSummaries,
) {
    let Some(binding) = statement
        .payload
        .as_ref()
        .and_then(|payload| payload.binding())
    else {
        return;
    };
    define_payload_borrow_return_binding(
        binding,
        &statement.expression,
        resolved,
        source_environment,
        payload_environment,
        borrow_provenance,
        summaries,
    );
}

pub(super) fn define_switch_arm_payload_borrow_return_binding(
    arm: &SwitchArm,
    target_expression: &Expr,
    resolved: &ResolveOutput,
    source_environment: &TypeEnvironment,
    payload_environment: &TypeEnvironment,
    borrow_provenance: &mut BorrowReturnEnvironment,
    summaries: &BorrowReturnSummaries,
) {
    let Some(binding) = arm.payload.as_ref().and_then(|payload| payload.binding()) else {
        return;
    };
    define_payload_borrow_return_binding(
        binding,
        target_expression,
        resolved,
        source_environment,
        payload_environment,
        borrow_provenance,
        summaries,
    );
}

pub(super) fn define_payload_borrow_return_binding(
    binding: &SwitchPayloadBinding,
    target_expression: &Expr,
    resolved: &ResolveOutput,
    source_environment: &TypeEnvironment,
    payload_environment: &TypeEnvironment,
    borrow_provenance: &mut BorrowReturnEnvironment,
    summaries: &BorrowReturnSummaries,
) {
    let Some(binding_type) = payload_environment.get(&binding.name) else {
        borrow_provenance.define_binding(binding.name.clone(), false, None);
        return;
    };
    let contains_borrow_like = type_contains_borrow_like(binding_type, resolved);
    let provenance = contains_borrow_like.then(|| {
        let target_type = expression_type(target_expression, resolved, source_environment);
        borrow_return_provenance_for_expression(
            target_expression,
            &target_type,
            resolved,
            source_environment,
            borrow_provenance,
            summaries,
        )
    });
    borrow_provenance.define_binding(
        binding.name.clone(),
        contains_borrow_like,
        provenance.flatten(),
    );
}

pub(super) fn method_receiver_is_borrow(method: &crate::resolve::MethodSignature) -> bool {
    matches!(&method.receiver.ty, TypeExpr::Borrow(_))
}

pub(super) fn borrow_return_provenance_for_borrowed_input(
    expression: &Expr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    borrow_provenance: &BorrowReturnEnvironment,
    summaries: &BorrowReturnSummaries,
) -> Option<BorrowReturnProvenance> {
    let ty = expression_type(expression, resolved, environment);
    if type_contains_borrow_like(&ty, resolved) {
        return borrow_return_provenance_for_expression(
            expression,
            &ty,
            resolved,
            environment,
            borrow_provenance,
            summaries,
        );
    }

    let Some(identifier) = expression_root_identifier(expression) else {
        return Some(BorrowReturnProvenance::escaping(
            "temporary expression".to_string(),
        ));
    };
    if environment
        .get(&identifier.name)
        .is_some_and(|ty| type_contains_borrow_like(ty, resolved))
    {
        return borrow_return_provenance_for_identifier(
            identifier,
            resolved,
            environment,
            borrow_provenance,
        );
    }

    borrow_return_provenance_for_local_storage(identifier, resolved)
}

pub(super) fn borrow_return_provenance_for_identifier(
    identifier: &crate::ast::IdentifierExpr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    borrow_provenance: &BorrowReturnEnvironment,
) -> Option<BorrowReturnProvenance> {
    if let Some(provenance) = borrow_provenance.get(&identifier.name) {
        return Some(provenance.clone());
    }

    if matches!(
        resolved.local_symbol_for_identifier(identifier)?.kind,
        LocalSymbolKind::Parameter
    ) && environment
        .get(&identifier.name)
        .is_some_and(|ty| type_contains_borrow_like(ty, resolved))
    {
        return Some(BorrowReturnProvenance::input_borrow(
            identifier.name.clone(),
        ));
    }

    None
}

pub(super) fn borrow_return_provenance_for_direct_borrow(
    expression: &Expr,
    resolved: &ResolveOutput,
) -> Option<BorrowReturnProvenance> {
    let Expr::Borrow(borrow) = unwrap_group(expression) else {
        return None;
    };

    let source = match unwrap_group(&borrow.expression) {
        Expr::Identifier(identifier) => {
            borrow_return_provenance_for_local_storage(identifier, resolved)?
                .escaping_source()?
                .to_string()
        }
        _ => "temporary expression".to_string(),
    };

    Some(BorrowReturnProvenance::escaping(source))
}

pub(super) fn borrow_return_provenance_for_local_storage(
    identifier: &crate::ast::IdentifierExpr,
    resolved: &ResolveOutput,
) -> Option<BorrowReturnProvenance> {
    let source = match resolved.local_symbol_for_identifier(identifier)?.kind {
        LocalSymbolKind::Parameter => format!("parameter `{}`", identifier.name),
        LocalSymbolKind::Binding(_) => format!("local binding `{}`", identifier.name),
        LocalSymbolKind::PatternPayload => format!("payload binding `{}`", identifier.name),
        LocalSymbolKind::CatchError => format!("catch binding `{}`", identifier.name),
        LocalSymbolKind::ForRange => format!("for-range binding `{}`", identifier.name),
    };

    Some(BorrowReturnProvenance::escaping(source))
}

pub(super) fn type_contains_borrow_like(ty: &Type, resolved: &ResolveOutput) -> bool {
    type_contains_borrow_like_inner(ty, resolved, &mut HashSet::new())
}

pub(super) fn type_contains_borrow_like_inner(
    ty: &Type,
    resolved: &ResolveOutput,
    resolving_names: &mut HashSet<String>,
) -> bool {
    match ty {
        Type::Str | Type::View { .. } => true,
        Type::Named(name) if name.starts_with('&') => true,
        Type::Named(name) => {
            type_symbol_contains_borrow_like(name, resolved, &HashMap::new(), resolving_names)
        }
        Type::Generic { name, arguments } => {
            let Some(symbol) = resolved.type_symbol_by_canonical_name(name) else {
                return false;
            };
            let substitutions = symbol
                .generic_parameters
                .iter()
                .cloned()
                .zip(arguments.iter().cloned())
                .collect();
            type_symbol_contains_borrow_like(name, resolved, &substitutions, resolving_names)
        }
        Type::Array { element, .. } | Type::Optional(element) => {
            type_contains_borrow_like_inner(element, resolved, resolving_names)
        }
        Type::Fallible { success, error } => {
            type_contains_borrow_like_inner(success, resolved, resolving_names)
                || type_contains_borrow_like_inner(error, resolved, resolving_names)
        }
        Type::ArrayData { element } => {
            type_contains_borrow_like_inner(element, resolved, resolving_names)
        }
        Type::Error => true,
        Type::I32
        | Type::Primitive(_)
        | Type::StrData
        | Type::Void
        | Type::Never
        | Type::None
        | Type::Pointer(_)
        | Type::Parameter(_)
        | Type::Unresolved(_)
        | Type::Unknown => false,
    }
}

pub(super) fn type_symbol_contains_borrow_like(
    canonical_name: &str,
    resolved: &ResolveOutput,
    substitutions: &HashMap<String, Type>,
    resolving_names: &mut HashSet<String>,
) -> bool {
    if !resolving_names.insert(canonical_name.to_string()) {
        return false;
    }

    let result = resolved
        .type_symbol_by_canonical_name(canonical_name)
        .is_some_and(|symbol| match symbol.kind {
            TypeSymbolKind::Alias => symbol.alias_target.as_ref().is_some_and(|target| {
                type_expr_contains_borrow_like(target, resolved, substitutions, resolving_names)
            }),
            TypeSymbolKind::Struct => symbol.fields.iter().any(|field| {
                type_expr_contains_borrow_like(&field.ty, resolved, substitutions, resolving_names)
            }),
            TypeSymbolKind::Enum => symbol.variants.iter().any(|variant| {
                variant.payload.iter().any(|payload| {
                    type_expr_contains_borrow_like(
                        &payload.ty,
                        resolved,
                        substitutions,
                        resolving_names,
                    )
                })
            }),
            TypeSymbolKind::Interface => false,
        });

    resolving_names.remove(canonical_name);
    result
}

pub(super) fn type_expr_contains_borrow_like(
    ty: &TypeExpr,
    resolved: &ResolveOutput,
    substitutions: &HashMap<String, Type>,
    resolving_names: &mut HashSet<String>,
) -> bool {
    match ty {
        TypeExpr::Borrow(_) => true,
        TypeExpr::View(view) => {
            type_expr_contains_borrow_like(&view.element, resolved, substitutions, resolving_names)
        }
        TypeExpr::Array(array) => {
            type_expr_contains_borrow_like(&array.element, resolved, substitutions, resolving_names)
        }
        TypeExpr::Optional(optional) => type_expr_contains_borrow_like(
            &optional.inner,
            resolved,
            substitutions,
            resolving_names,
        ),
        TypeExpr::Fallible(fallible) => {
            type_expr_contains_borrow_like(
                &fallible.success,
                resolved,
                substitutions,
                resolving_names,
            ) || type_expr_contains_borrow_like(
                &fallible.error,
                resolved,
                substitutions,
                resolving_names,
            )
        }
        TypeExpr::Pointer(_) => false,
        TypeExpr::Reference(reference) => {
            if reference.name == "error" {
                return true;
            }
            substitutions
                .get(&reference.name)
                .is_some_and(|ty| type_contains_borrow_like_inner(ty, resolved, resolving_names))
                || resolved
                    .type_symbol_by_reference_name(&reference.name)
                    .is_some_and(|symbol| {
                        type_symbol_contains_borrow_like(
                            &symbol.canonical_name,
                            resolved,
                            &HashMap::new(),
                            resolving_names,
                        )
                    })
        }
        TypeExpr::Generic(generic) => {
            if let Some(ty) = substitutions.get(&generic.name) {
                return type_contains_borrow_like_inner(ty, resolved, resolving_names);
            }
            let Some(symbol) = resolved.type_symbol_by_reference_name(&generic.name) else {
                return false;
            };
            let nested_substitutions = symbol
                .generic_parameters
                .iter()
                .cloned()
                .zip(generic.arguments.iter().map(|argument| {
                    type_expr_to_type_with_substitutions(argument, resolved, None, substitutions)
                }))
                .collect();
            type_symbol_contains_borrow_like(
                &symbol.canonical_name,
                resolved,
                &nested_substitutions,
                resolving_names,
            )
        }
    }
}
