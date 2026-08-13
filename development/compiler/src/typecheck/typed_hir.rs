//! Identity-keyed, error-tolerant expression semantics.

use crate::ast::TypeExpr;
use crate::integer::IntegerType;
use crate::semantic::{BodyId, ExprId, SemanticDb, TyId};
use crate::source::ByteSpan;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PartialSemantic<T> {
    Known(T),
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TypedExpression {
    pub(crate) id: ExprId,
    pub(crate) body: BodyId,
    pub(crate) ty: PartialSemantic<TyId>,
    pub(crate) contextual_ty: Option<TyId>,
    pub(crate) diverges: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CheckedScalarType {
    Integer(IntegerType),
    Bool,
}

/// Type arena and expression table for one immutable semantic generation.
///
/// Source spans are accepted only at the syntax-to-semantics boundary. Stored
/// expression facts are keyed by `ExprId`; equal type structure shares a
/// `TyId` within this checked file.
#[derive(Debug, Clone)]
pub(super) struct TypedExpressionArena {
    semantic_db: Arc<SemanticDb>,
    types: Vec<TypeExpr>,
    scalar_types: Vec<Option<CheckedScalarType>>,
    type_ids: HashMap<String, TyId>,
    expressions: HashMap<ExprId, TypedExpression>,
}

impl TypedExpressionArena {
    pub(super) fn new(semantic_db: Arc<SemanticDb>, anchor: ByteSpan) -> Self {
        let mut arena = Self {
            semantic_db,
            types: Vec::new(),
            scalar_types: Vec::new(),
            type_ids: HashMap::new(),
            expressions: HashMap::new(),
        };
        for (name, scalar) in [
            ("bool", CheckedScalarType::Bool),
            ("i32", CheckedScalarType::Integer(IntegerType::I32)),
            ("usize", CheckedScalarType::Integer(IntegerType::Usize)),
        ] {
            arena.intern_type(
                TypeExpr::Reference(crate::ast::TypeReference {
                    span: anchor,
                    name: name.to_string(),
                }),
                Some(scalar),
            );
        }
        arena
    }

    pub(super) fn record_type(
        &mut self,
        expression_span: ByteSpan,
        ty: Option<TypeExpr>,
        scalar: Option<CheckedScalarType>,
        diverges: bool,
    ) {
        let Some(expression) = self.semantic_db.expression_at(expression_span) else {
            return;
        };
        let Some(definition) = self.semantic_db.expression(expression) else {
            return;
        };
        debug_assert_eq!(definition.id, expression);
        debug_assert_eq!(definition.span, expression_span);
        let body = definition.body;
        let ty = match ty {
            Some(ty) => PartialSemantic::Known(self.intern_type(ty, scalar)),
            None => PartialSemantic::Error,
        };
        let diverges = diverges
            || self
                .expressions
                .get(&expression)
                .is_some_and(|expression| expression.diverges);
        self.expressions.insert(
            expression,
            TypedExpression {
                id: expression,
                body,
                ty,
                contextual_ty: None,
                diverges,
            },
        );
    }

    pub(super) fn record_contextual_type(
        &mut self,
        expression_span: ByteSpan,
        ty: TypeExpr,
        scalar: Option<CheckedScalarType>,
    ) {
        let Some(expression) = self.semantic_db.expression_at(expression_span) else {
            return;
        };
        let ty = self.intern_type(ty, scalar);
        let Some(expression) = self.expressions.get_mut(&expression) else {
            return;
        };
        debug_assert!(
            expression
                .contextual_ty
                .is_none_or(|existing| existing == ty)
        );
        expression.contextual_ty = Some(ty);
    }

    fn intern_type(&mut self, ty: TypeExpr, scalar: Option<CheckedScalarType>) -> TyId {
        let key = crate::ast::canonical_type_expr(&ty);
        if let Some(id) = self.type_ids.get(&key) {
            let recorded = &mut self.scalar_types[id.index()];
            debug_assert!(recorded.is_none() || scalar.is_none() || *recorded == scalar);
            if recorded.is_none() {
                *recorded = scalar;
            }
            return *id;
        }
        let id = TyId::from_index(self.types.len());
        self.types.push(ty);
        self.scalar_types.push(scalar);
        self.type_ids.insert(key, id);
        id
    }

    pub(super) fn expression_id_at(&self, span: ByteSpan) -> Option<ExprId> {
        self.semantic_db.expression_at(span)
    }

    pub(super) fn expression(&self, expression: ExprId) -> Option<&TypedExpression> {
        self.expressions.get(&expression)
    }

    pub(super) fn type_expr(&self, ty: TyId) -> Option<&TypeExpr> {
        self.types.get(ty.index())
    }

    pub(super) fn type_id(&self, ty: &TypeExpr) -> Option<TyId> {
        self.type_ids
            .get(&crate::ast::canonical_type_expr(ty))
            .copied()
    }

    pub(super) fn scalar_type(&self, ty: TyId) -> Option<CheckedScalarType> {
        self.scalar_types.get(ty.index()).copied().flatten()
    }
}
