//! Analysis-time index of syntax-defined completion contexts.

use crate::ast::{AstFile, Expr, Stmt, StructLiteralExpr, TypeExpr};
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
    query_span: ByteSpan,
    start_is_exclusive: bool,
    end_is_inclusive: bool,
    excluded_spans: Vec<ByteSpan>,
    kind: CompletionSiteKind,
}

impl CompletionSite {
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

#[derive(Debug, Clone, Default)]
pub(crate) struct CompletionSiteIndex {
    sites: Vec<CompletionSite>,
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
        index.sites.sort_by_key(|site| {
            (
                site.query_span.start,
                site.query_span.end,
                site_priority(&site.kind),
            )
        });
        index
    }

    pub(super) fn at_offset(&self, offset: usize) -> Option<&CompletionSiteKind> {
        self.sites
            .iter()
            .filter(|site| site.matches(offset))
            .min_by_key(|site| (site.query_span.len(), site_priority(&site.kind)))
            .map(|site| &site.kind)
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
                query_span: expression.fields_span,
                start_is_exclusive: false,
                end_is_inclusive: false,
                excluded_spans: expression
                    .fields
                    .iter()
                    .map(|field| field.value.span())
                    .collect(),
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
            query_span,
            start_is_exclusive,
            end_is_inclusive: true,
            excluded_spans: Vec::new(),
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
