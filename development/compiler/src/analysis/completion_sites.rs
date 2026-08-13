//! Analysis-time index of syntax-defined completion contexts.

use crate::ast::{
    AstFile, Expr, InstanceDecl, Item, Stmt, StructLiteralExpr, TypeExpr, WhereClause,
};
use crate::source::ByteSpan;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum CompletionSiteKind {
    LiteralShape(TypeExpr),
    RegionAllocator,
    EnumPatternMembers(String),
    MemberAccess {
        owner_name: String,
        owner_span: ByteSpan,
    },
    StructLiteralFields(StructLiteralExpr),
}

#[derive(Debug, Clone)]
struct CompletionSite {
    region: CompletionRegion,
    kind: CompletionSiteKind,
}

#[derive(Debug, Clone)]
struct CompletionRegion {
    query_span: ByteSpan,
    start_is_exclusive: bool,
    end_is_inclusive: bool,
    excluded_spans: Vec<ByteSpan>,
}

impl CompletionRegion {
    fn matches(&self, offset: usize) -> bool {
        let starts_before_cursor = if self.start_is_exclusive {
            self.query_span.start < offset
        } else {
            self.query_span.start <= offset
        };
        let ends_after_cursor = if self.end_is_inclusive {
            offset <= self.query_span.end
        } else {
            offset < self.query_span.end
        };
        starts_before_cursor
            && ends_after_cursor
            && !self
                .excluded_spans
                .iter()
                .any(|span| contains(*span, offset))
    }
}

#[derive(Debug, Clone)]
struct InstanceDeclarationSite {
    region: CompletionRegion,
    instance: InstanceDecl,
}

#[derive(Debug, Clone)]
struct RequirementGap {
    clause_span: ByteSpan,
    predicate_spans: Vec<ByteSpan>,
}

impl RequirementGap {
    fn matches(&self, offset: usize) -> bool {
        contains_or_touches(self.clause_span, offset)
            && !self
                .predicate_spans
                .iter()
                .any(|span| span.start < offset && offset <= span.end)
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct CompletionSiteIndex {
    sites: Vec<CompletionSite>,
    requirement_gaps: Vec<RequirementGap>,
    operator_requirements: Vec<ByteSpan>,
    instance_declaration_slots: Vec<InstanceDeclarationSite>,
}

impl CompletionSiteIndex {
    pub(crate) fn new(ast: &AstFile) -> Self {
        let mut index = Self::default();
        crate::ast::visit_file_expressions(ast, &mut |expression| {
            index.collect_expression(expression);
        });
        crate::ast::visit_file_statements(ast, &mut |statement| {
            index.collect_statement(statement);
        });
        for item in &ast.items {
            index.collect_item(item);
        }
        index.sites.sort_by_key(|site| {
            (
                site.region.query_span.start,
                site.region.query_span.end,
                site_priority(&site.kind),
            )
        });
        index
    }

    pub(super) fn at_offset(&self, offset: usize) -> Option<&CompletionSiteKind> {
        self.sites
            .iter()
            .filter(|site| site.region.matches(offset))
            .min_by_key(|site| (site.region.query_span.len(), site_priority(&site.kind)))
            .map(|site| &site.kind)
    }

    pub(super) fn copy_requirement_is_allowed(&self, offset: usize) -> bool {
        self.requirement_gaps.iter().any(|gap| gap.matches(offset))
    }

    pub(super) fn operator_requirement_contains(&self, offset: usize) -> bool {
        self.operator_requirements
            .iter()
            .any(|span| contains_or_touches(*span, offset))
    }

    pub(super) fn instance_declaration_at(&self, offset: usize) -> Option<&InstanceDecl> {
        self.instance_declaration_slots
            .iter()
            .filter(|site| site.region.matches(offset))
            .min_by_key(|site| site.region.query_span.len())
            .map(|site| &site.instance)
    }

    fn collect_item(&mut self, item: &Item) {
        for clause in item_requirement_clauses(item) {
            self.requirement_gaps.push(RequirementGap {
                clause_span: clause.span,
                predicate_spans: clause.predicates.iter().map(where_predicate_span).collect(),
            });
            self.operator_requirements
                .extend(clause.operator_requirements().map(|requirement| {
                    ByteSpan::new(
                        requirement.span.source,
                        requirement.open_paren_span.end,
                        requirement.span.end,
                    )
                }));
        }
        if let Item::Instance(instance) = item {
            self.instance_declaration_slots
                .push(InstanceDeclarationSite {
                    region: CompletionRegion {
                        query_span: ByteSpan::new(
                            instance.span.source,
                            instance.target_ty.span().end,
                            instance.span.end,
                        ),
                        start_is_exclusive: true,
                        end_is_inclusive: false,
                        excluded_spans: instance
                            .callables()
                            .map(|callable| callable.span)
                            .collect(),
                    },
                    instance: instance.clone(),
                });
        }
    }

    fn collect_expression(&mut self, expression: &Expr) {
        match expression {
            Expr::TypedSequenceLiteral(expression) => self.inclusive_site(
                ByteSpan::new(
                    expression.target.span().source,
                    expression.target.span().end,
                    expression.elements_span.start,
                ),
                false,
                CompletionSiteKind::LiteralShape(expression.target.clone()),
            ),
            Expr::TypedStringLiteral(expression) => self.inclusive_site(
                ByteSpan::new(
                    expression.target.span().source,
                    expression.target.span().end,
                    expression.text.span.start,
                ),
                false,
                CompletionSiteKind::LiteralShape(expression.target.clone()),
            ),
            Expr::StructLiteral(expression) => self.sites.push(CompletionSite {
                region: CompletionRegion {
                    query_span: expression.fields_span,
                    start_is_exclusive: false,
                    end_is_inclusive: false,
                    excluded_spans: expression
                        .fields
                        .iter()
                        .map(|field| field.value.span())
                        .collect(),
                },
                kind: CompletionSiteKind::StructLiteralFields(expression.clone()),
            }),
            Expr::Member(expression) => {
                let Expr::Identifier(owner) = expression.object.without_groups() else {
                    return;
                };
                self.inclusive_site(
                    ByteSpan::new(
                        owner.span.source,
                        owner.span.end,
                        expression.member_span.end,
                    ),
                    true,
                    CompletionSiteKind::MemberAccess {
                        owner_name: owner.name.clone(),
                        owner_span: owner.span,
                    },
                );
            }
            Expr::IfIs(expression) => self.enum_pattern(
                expression.enum_name_span,
                expression.variant_name_span,
                &expression.enum_name,
            ),
            Expr::Match(expression) => {
                for arm in &expression.arms {
                    self.enum_pattern(arm.enum_name_span, arm.variant_name_span, &arm.enum_name);
                }
            }
            Expr::Closure(_)
            | Expr::Identifier(_)
            | Expr::IntegerLiteral(_)
            | Expr::ByteLiteral(_)
            | Expr::StringLiteral(_)
            | Expr::BoolLiteral(_)
            | Expr::NoneLiteral(_)
            | Expr::InterpolatedString(_)
            | Expr::ArrayLiteral(_)
            | Expr::Propagate(_)
            | Expr::Force(_)
            | Expr::Catch(_)
            | Expr::Borrow(_)
            | Expr::Unary(_)
            | Expr::Binary(_)
            | Expr::TypeConversion(_)
            | Expr::Call(_)
            | Expr::Index(_)
            | Expr::Group(_)
            | Expr::Otherwise(_)
            | Expr::If(_) => {}
        }
    }

    fn collect_statement(&mut self, statement: &Stmt) {
        match statement {
            Stmt::IfIs(statement) => self.enum_pattern(
                statement.enum_name_span,
                statement.variant_name_span,
                &statement.enum_name,
            ),
            Stmt::Switch(statement) => {
                for arm in &statement.arms {
                    self.enum_pattern(arm.enum_name_span, arm.variant_name_span, &arm.enum_name);
                }
            }
            Stmt::Region(statement) => self.inclusive_site(
                statement.allocator.span(),
                false,
                CompletionSiteKind::RegionAllocator,
            ),
            Stmt::Return(_)
            | Stmt::Binding(_)
            | Stmt::Assignment(_)
            | Stmt::Import(_)
            | Stmt::FromImport(_)
            | Stmt::If(_)
            | Stmt::ForRange(_)
            | Stmt::CollectionFor(_)
            | Stmt::LiteralPackFor(_)
            | Stmt::While(_)
            | Stmt::Loop(_)
            | Stmt::Break(_)
            | Stmt::Continue(_)
            | Stmt::Drop(_)
            | Stmt::Expression(_) => {}
        }
    }

    fn enum_pattern(&mut self, owner: ByteSpan, member: ByteSpan, name: &str) {
        self.inclusive_site(
            ByteSpan::new(owner.source, owner.end, member.end),
            true,
            CompletionSiteKind::EnumPatternMembers(name.to_string()),
        );
    }

    fn inclusive_site(
        &mut self,
        query_span: ByteSpan,
        start_is_exclusive: bool,
        kind: CompletionSiteKind,
    ) {
        self.sites.push(CompletionSite {
            region: CompletionRegion {
                query_span,
                start_is_exclusive,
                end_is_inclusive: true,
                excluded_spans: Vec::new(),
            },
            kind,
        });
    }
}

const fn site_priority(kind: &CompletionSiteKind) -> u8 {
    match kind {
        CompletionSiteKind::RegionAllocator => 1,
        CompletionSiteKind::LiteralShape(_)
        | CompletionSiteKind::EnumPatternMembers(_)
        | CompletionSiteKind::MemberAccess { .. }
        | CompletionSiteKind::StructLiteralFields(_) => 0,
    }
}

const fn contains(span: ByteSpan, offset: usize) -> bool {
    span.start <= offset && offset < span.end
}

const fn contains_or_touches(span: ByteSpan, offset: usize) -> bool {
    span.start <= offset && offset <= span.end
}

fn where_predicate_span(predicate: &crate::ast::WherePredicate) -> ByteSpan {
    match predicate {
        crate::ast::WherePredicate::Copy(requirement) => requirement.span,
        crate::ast::WherePredicate::Generic(requirement) => requirement.span,
        crate::ast::WherePredicate::Refinement(refinement) => refinement.span,
        crate::ast::WherePredicate::Equality(equality) => equality.span,
        crate::ast::WherePredicate::Operator(requirement) => requirement.span,
        crate::ast::WherePredicate::Coercion(requirement) => requirement.span,
    }
}

pub(super) fn item_requirement_clauses(item: &Item) -> Vec<&WhereClause> {
    match item {
        Item::Function(function) => function.requirements.iter().collect(),
        Item::Primitive(primitive) => primitive.requirements.iter().collect(),
        Item::TypeAlias(alias) => alias.requirements.iter().collect(),
        Item::Struct(struct_) => struct_.requirements.iter().collect(),
        Item::Enum(enum_) => enum_.requirements.iter().collect(),
        Item::Interface(interface) => interface
            .requirements
            .iter()
            .chain(
                interface
                    .methods
                    .iter()
                    .filter_map(|method| method.requirements.as_ref()),
            )
            .collect(),
        Item::Instance(_) | Item::Conformance(_) => {
            let owner = item.method_owner().expect("matched method owner");
            owner
                .requirements()
                .into_iter()
                .chain(
                    owner
                        .methods()
                        .filter_map(|method| method.requirements.as_ref()),
                )
                .collect()
        }
        Item::Construct(construct) => construct
            .functions()
            .filter_map(|(_, function)| function.requirements.as_ref())
            .chain(
                construct
                    .literals()
                    .filter_map(|(_, literal)| literal.requirements.as_ref()),
            )
            .collect(),
        Item::Import(_) | Item::FromImport(_) | Item::Test(_) | Item::Destruct(_) => Vec::new(),
    }
}
