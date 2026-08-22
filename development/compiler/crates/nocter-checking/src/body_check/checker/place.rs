use nocter_model::{BorrowCapability, BuiltinType, TypeId, TypeKind};
use nocter_source_index::{SemanticEntity, SourceAccess, SourceOrigin, SyntaxOrigin};
use nocter_syntax::{NodeId, NodeKind, SyntaxToken};

use super::{BodyChecker, NodeProjection, ResolvedPlace};
use crate::body_check::diagnostic::BodyRule;
use crate::body_check::error::{BodyCheckError, BodyCheckInternalError};
use crate::field_selection::{FieldSelectionError, select_field};
use crate::instance_operations::{IndexOperationCandidate, retain_direct_candidates};
use crate::syntax::{
    direct_child, direct_identifier, direct_nodes, identifier_tokens, is_transparent_expression,
};
use crate::{LocalBindingKind, NameTarget, PlaceAccess, PlaceProjection, PlaceRoot};

struct PlaceDraft {
    root: PlaceRoot,
    ty: TypeId,
    access: PlaceAccess,
    writable: bool,
    projections: Vec<PlaceProjection>,
    projection_types: Vec<TypeId>,
    partial_parents: Vec<nocter_model::NominalTypeId>,
    source_projections: Vec<NodeProjection>,
}

impl BodyChecker<'_, '_> {
    pub(super) fn named_place(&mut self, node: NodeId) -> Result<ResolvedPlace, BodyCheckError> {
        let tokens = identifier_tokens(self.tree(), node);
        let root = tokens
            .first()
            .copied()
            .ok_or(BodyCheckInternalError::InvalidSyntax(node))?;
        let mut draft = self.start_place(node, root)?;
        for field in tokens.into_iter().skip(1) {
            self.push_field(node, &mut draft, field)?;
        }
        Ok(self.finish_place(draft))
    }

    pub(super) fn postfix_place(
        &mut self,
        node: NodeId,
        capability: BorrowCapability,
    ) -> Result<ResolvedPlace, BodyCheckError> {
        let syntax = match collect_postfix_operations(self.tree(), node) {
            Ok(syntax) => syntax,
            Err(PlaceSyntaxError::NotPlace(invalid)) => {
                let kind = self
                    .tree()
                    .node(invalid)
                    .map(nocter_syntax::SyntaxNode::kind)
                    .ok_or(BodyCheckInternalError::InvalidSyntax(invalid))?;
                return Err(BodyCheckInternalError::UnsupportedSyntax(invalid, kind).into());
            }
            Err(PlaceSyntaxError::InvalidSyntax(invalid)) => {
                return Err(BodyCheckInternalError::InvalidSyntax(invalid).into());
            }
        };
        self.resolve_place_syntax(syntax, capability)
    }

    pub(super) fn assignment_place(
        &mut self,
        node: NodeId,
        diagnostic_node: NodeId,
        invalid_rule: BodyRule,
    ) -> Result<ResolvedPlace, BodyCheckError> {
        let syntax = match collect_postfix_operations(self.tree(), node) {
            Ok(syntax) => syntax,
            Err(PlaceSyntaxError::NotPlace(_)) => {
                return Err(self.rule(invalid_rule, diagnostic_node)?);
            }
            Err(PlaceSyntaxError::InvalidSyntax(invalid)) => {
                return Err(BodyCheckInternalError::InvalidSyntax(invalid).into());
            }
        };
        self.resolve_place_syntax(syntax, BorrowCapability::ReadWrite)
    }

    fn resolve_place_syntax(
        &mut self,
        syntax: PlaceSyntax,
        capability: BorrowCapability,
    ) -> Result<ResolvedPlace, BodyCheckError> {
        let mut draft = if self.kind(syntax.root)? == NodeKind::ReferenceExpression {
            let token = direct_identifier(self.tree(), syntax.root)
                .ok_or(BodyCheckInternalError::InvalidSyntax(syntax.root))?;
            self.start_place(syntax.root, token)?
        } else {
            if matches!(
                self.kind(syntax.root)?,
                NodeKind::StructLiteral
                    | NodeKind::TypedSequenceLiteral
                    | NodeKind::TypedStringLiteral
                    | NodeKind::ArrayLiteral
                    | NodeKind::StringExpression
                    | NodeKind::ScalarLiteral
                    | NodeKind::ClosureExpression
            ) {
                return Err(BodyCheckInternalError::UnsupportedSyntax(
                    syntax.root,
                    self.kind(syntax.root)?,
                )
                .into());
            }
            self.start_value_place(syntax.root)?
        };
        for operation in syntax.operations {
            match operation {
                PlaceOperation::Field(suffix) => {
                    let Some(field) = direct_identifier(self.tree(), suffix) else {
                        let receiver = self.place_projection_base(&mut draft)?;
                        let origin = SourceOrigin::from_node(self.tree(), suffix)
                            .map_err(|_| BodyCheckInternalError::InvalidSyntax(suffix))?;
                        let available = if draft.writable {
                            BorrowCapability::ReadWrite
                        } else {
                            BorrowCapability::Readonly
                        };
                        self.record_member_interruption_origin(
                            origin,
                            receiver,
                            available,
                            draft.access == PlaceAccess::Owned,
                        );
                        return Err(BodyCheckInternalError::InvalidSyntax(suffix).into());
                    };
                    self.push_field(suffix, &mut draft, field)?;
                }
                PlaceOperation::Index(suffix) => self.push_index(suffix, &mut draft, capability)?,
            }
        }
        Ok(self.finish_place(draft))
    }

    pub(super) fn is_writable_place(
        &self,
        place: nocter_model::PlaceId,
    ) -> Result<bool, BodyCheckInternalError> {
        let place = self
            .builder
            .place(place)
            .ok_or(BodyCheckInternalError::InvalidMovePlace(place))?;
        Ok(place.is_writable())
    }

    pub(super) fn reborrow_place_value(
        &mut self,
        syntax: NodeId,
        place: nocter_model::PlaceId,
        target_capability: BorrowCapability,
    ) -> Result<nocter_model::BodyNodeId, BodyCheckError> {
        let checked = self
            .builder
            .place(place)
            .cloned()
            .ok_or(BodyCheckInternalError::InvalidMovePlace(place))?;
        let Some(TypeKind::Borrow {
            capability: source_capability,
            referent,
        }) = self.types.get(checked.ty()).cloned()
        else {
            return Err(self.rule(BodyRule::TypeMismatch, syntax)?);
        };
        let mut draft = PlaceDraft {
            root: checked.root(),
            ty: checked.ty(),
            access: checked.access(),
            writable: checked.is_writable(),
            projections: checked.projections().to_vec(),
            projection_types: checked.projection_types().to_vec(),
            partial_parents: Vec::new(),
            source_projections: Vec::new(),
        };
        let base = self.place_projection_base(&mut draft)?;
        if base != referent
            || (target_capability == BorrowCapability::ReadWrite
                && (source_capability != BorrowCapability::ReadWrite || !draft.writable))
        {
            return Err(self.rule(BodyRule::InvalidReadWriteBorrow, syntax)?);
        }
        draft.ty = base;
        let place = self.finish_place(draft);
        let ty = self
            .types
            .intern(TypeKind::Borrow {
                capability: target_capability,
                referent,
            })
            .map_err(|_| BodyCheckInternalError::UnknownType(referent))?;
        self.add_node(
            syntax,
            ty,
            crate::CheckedOperation::Borrow {
                capability: target_capability,
                place: place.id,
            },
        )
    }

    pub(super) fn is_region_place(
        &self,
        place: nocter_model::PlaceId,
    ) -> Result<bool, BodyCheckInternalError> {
        let root = self
            .builder
            .place(place)
            .map(crate::CheckedPlace::root)
            .ok_or(BodyCheckInternalError::InvalidMovePlace(place))?;
        Ok(self.is_region_root(root))
    }

    pub(super) fn is_region_root(&self, root: PlaceRoot) -> bool {
        matches!(
            root,
            PlaceRoot::Local(local)
                if self
                    .names
                    .locals()
                    .get(local)
                    .is_some_and(|local| local.kind() == LocalBindingKind::Region)
        )
    }

    fn start_place(
        &mut self,
        node: NodeId,
        token: SyntaxToken,
    ) -> Result<PlaceDraft, BodyCheckError> {
        let (root, ty) = self.place_root(node, token)?;
        self.start_place_from_root(node, root, ty)
    }

    fn start_value_place(&mut self, node: NodeId) -> Result<PlaceDraft, BodyCheckError> {
        let value = self.check_expression(node, None)?;
        let ty = self.node_type(value)?;
        if !matches!(self.types.get(ty), Some(TypeKind::Borrow { .. })) {
            return Err(self.rule(BodyRule::InvalidIndexOperation, node)?);
        }
        self.start_place_from_root(node, PlaceRoot::Value(value), ty)
    }

    pub(super) fn target_place(
        &mut self,
        node: NodeId,
        target: NameTarget,
    ) -> Result<ResolvedPlace, BodyCheckError> {
        let (root, ty) = self.place_root_for_target(node, target)?;
        let draft = self.start_place_from_root(node, root, ty)?;
        Ok(self.finish_place(draft))
    }

    fn start_place_from_root(
        &self,
        node: NodeId,
        root: PlaceRoot,
        ty: TypeId,
    ) -> Result<PlaceDraft, BodyCheckError> {
        let writable = match root {
            PlaceRoot::Local(local) => self
                .names
                .locals()
                .get(local)
                .is_some_and(|local| local.kind() == LocalBindingKind::Mutable),
            PlaceRoot::Capture(capture) => self
                .names
                .captures()
                .get(capture)
                .is_some_and(|capture| capture.mode() == crate::CaptureMode::ReadWrite),
            PlaceRoot::Parameter(_) | PlaceRoot::Value(_) => false,
        };
        let access = match root {
            PlaceRoot::Capture(capture) => match self
                .names
                .captures()
                .get(capture)
                .map(|capture| capture.mode())
            {
                Some(crate::CaptureMode::Readonly) => {
                    PlaceAccess::Borrowed(BorrowCapability::Readonly)
                }
                Some(crate::CaptureMode::ReadWrite) => {
                    PlaceAccess::Borrowed(BorrowCapability::ReadWrite)
                }
                Some(crate::CaptureMode::Move) => PlaceAccess::Owned,
                None => {
                    return Err(BodyCheckInternalError::UnsupportedNameTarget(
                        node,
                        NameTarget::Capture(capture),
                    )
                    .into());
                }
            },
            PlaceRoot::Parameter(_) | PlaceRoot::Local(_) | PlaceRoot::Value(_) => {
                PlaceAccess::Owned
            }
        };
        Ok(PlaceDraft {
            root,
            ty,
            access,
            writable,
            projections: Vec::new(),
            projection_types: Vec::new(),
            partial_parents: Vec::new(),
            source_projections: Vec::new(),
        })
    }

    fn push_field(
        &mut self,
        node: NodeId,
        draft: &mut PlaceDraft,
        field_token: SyntaxToken,
    ) -> Result<(), BodyCheckError> {
        let base = self.place_projection_base(draft)?;
        let spelling = self.token_text(field_token)?.to_owned();
        let selected = match select_field(
            self.graph,
            self.types,
            self.source.module(),
            base,
            &spelling,
        ) {
            Ok(selected) => selected,
            Err(FieldSelectionError::NoFields(_) | FieldSelectionError::MissingField(_)) => {
                return Err(self.token_rule(BodyRule::UnknownField, field_token)?);
            }
            Err(FieldSelectionError::InaccessibleField(_)) => {
                return Err(self.token_rule(BodyRule::InaccessibleField, field_token)?);
            }
            Err(FieldSelectionError::UnknownType(unknown)) => {
                return Err(BodyCheckInternalError::UnknownType(unknown).into());
            }
            Err(
                FieldSelectionError::UnknownNominal(_)
                | FieldSelectionError::UnknownField(_)
                | FieldSelectionError::UnknownFieldSite(_)
                | FieldSelectionError::AmbiguousField(_)
                | FieldSelectionError::GenericArity(_)
                | FieldSelectionError::Substitution(_)
                | FieldSelectionError::UnknownBorrowType(_),
            ) => return Err(BodyCheckInternalError::FieldSelection.into()),
        };
        if draft.access == PlaceAccess::Owned
            && let Some(owner) = selected.owner()
        {
            draft.partial_parents.push(owner);
        }
        let origin = SourceOrigin::from_token(self.tree(), field_token)
            .map_err(|_| BodyCheckInternalError::InvalidSyntax(node))?;
        let entity = match selected.field() {
            nocter_model::FieldIdentity::Declared(field) => SemanticEntity::Field(field),
            nocter_model::FieldIdentity::Builtin(field) => SemanticEntity::BuiltinField(field),
        };
        draft
            .source_projections
            .push(NodeProjection::new(entity, origin));
        draft
            .projections
            .push(PlaceProjection::Field(selected.field()));
        draft.ty = selected.ty();
        draft.projection_types.push(draft.ty);
        Ok(())
    }

    fn push_index(
        &mut self,
        suffix: NodeId,
        draft: &mut PlaceDraft,
        capability: BorrowCapability,
    ) -> Result<(), BodyCheckError> {
        let base = self.place_projection_base(draft)?;
        let builtin = match self.types.get(base) {
            Some(TypeKind::FixedArray { element, .. } | TypeKind::Slice(element)) => Some(*element),
            Some(TypeKind::Builtin(BuiltinType::Str)) => {
                draft.access = PlaceAccess::Borrowed(BorrowCapability::Readonly);
                draft.writable = false;
                Some(self.types.builtin(BuiltinType::U8))
            }
            Some(_) => None,
            None => return Err(BodyCheckInternalError::UnknownType(base).into()),
        };
        let expression = direct_child(self.tree(), suffix, NodeKind::Expression)
            .ok_or(BodyCheckInternalError::InvalidSyntax(suffix))?;
        if let Some(element) = builtin {
            let index =
                self.check_expression(expression, Some(self.types.builtin(BuiltinType::Usize)))?;
            draft
                .projections
                .push(PlaceProjection::BuiltinIndex { index });
            draft.ty = element;
            draft.projection_types.push(draft.ty);
            return Ok(());
        }
        let receiver_writable = draft.writable;
        let mut candidates = {
            let mut selector = self.instance_selector();
            let mut candidates = selector
                .select_index_operations(base, capability)
                .map_err(BodyCheckInternalError::from)?;
            candidates.extend(
                selector
                    .select_coerced_index_operations(base, capability)
                    .map_err(BodyCheckInternalError::from)?,
            );
            candidates
        };
        retain_direct_candidates(&mut candidates);
        let expected = candidates
            .first()
            .map(IndexOperationCandidate::index)
            .filter(|expected| {
                candidates
                    .iter()
                    .all(|candidate| candidate.index() == *expected)
            });
        let index = self.check_expression(expression, expected)?;
        let index_ty = self.node_type(index)?;
        let candidates = candidates
            .into_iter()
            .filter(|candidate| candidate.index() == index_ty)
            .collect::<Vec<_>>();
        let mut candidates = candidates.into_iter();
        let Some(selected) = candidates.next() else {
            return Err(self.rule(BodyRule::InvalidIndexOperation, suffix)?);
        };
        if candidates.next().is_some() {
            return Err(self.rule(BodyRule::InvalidIndexOperation, suffix)?);
        }
        match (selected.operation(), selected.receiver_coercion()) {
            (Some(operation), receiver_coercion) => {
                draft.projections.push(PlaceProjection::SelectedIndex {
                    index,
                    operation: operation.clone(),
                    receiver_coercion: receiver_coercion.cloned(),
                });
            }
            (None, Some(receiver_coercion)) => {
                draft
                    .projections
                    .push(PlaceProjection::CoercedBuiltinIndex {
                        index,
                        receiver_coercion: receiver_coercion.clone(),
                    });
            }
            (None, None) => return Err(BodyCheckInternalError::IndexSelection.into()),
        }
        draft.ty = selected.result();
        draft.projection_types.push(draft.ty);
        draft.access = PlaceAccess::Borrowed(capability);
        draft.writable = capability == BorrowCapability::ReadWrite && receiver_writable;
        Ok(())
    }

    fn place_projection_base(&self, draft: &mut PlaceDraft) -> Result<TypeId, BodyCheckError> {
        let mut ty = draft.ty;
        loop {
            match self.types.get(ty) {
                Some(TypeKind::Borrow {
                    capability,
                    referent,
                }) => {
                    draft.projections.push(PlaceProjection::BorrowDeref {
                        capability: *capability,
                    });
                    draft.access = PlaceAccess::Borrowed(match (draft.access, *capability) {
                        (PlaceAccess::Borrowed(BorrowCapability::Readonly), _)
                        | (_, BorrowCapability::Readonly) => BorrowCapability::Readonly,
                        (
                            PlaceAccess::Owned | PlaceAccess::Borrowed(BorrowCapability::ReadWrite),
                            BorrowCapability::ReadWrite,
                        ) => BorrowCapability::ReadWrite,
                    });
                    draft.writable = matches!(
                        draft.access,
                        PlaceAccess::Borrowed(BorrowCapability::ReadWrite)
                    );
                    ty = *referent;
                    draft.projection_types.push(ty);
                }
                Some(_) => return Ok(ty),
                None => return Err(BodyCheckInternalError::UnknownType(ty).into()),
            }
        }
    }

    fn finish_place(&mut self, draft: PlaceDraft) -> ResolvedPlace {
        let access = if draft.writable {
            SourceAccess::Writable
        } else {
            SourceAccess::Readonly
        };
        self.projections.extend(
            draft
                .source_projections
                .into_iter()
                .map(|projection| projection.with_access(access)),
        );
        ResolvedPlace {
            id: self.builder.add_place(
                draft.root,
                draft.projections,
                draft.projection_types,
                draft.ty,
                draft.access,
                draft.writable,
            ),
            ty: draft.ty,
            access: draft.access,
            partial_parents: draft.partial_parents.into_boxed_slice(),
        }
    }

    pub(super) fn place_root(
        &mut self,
        node: NodeId,
        token: SyntaxToken,
    ) -> Result<(PlaceRoot, TypeId), BodyCheckError> {
        let origin = SyntaxOrigin::Token(token);
        let target = self
            .uses
            .get(&origin)
            .copied()
            .ok_or(BodyCheckInternalError::MissingNameUse(node))?;
        self.consumed_uses.insert(origin);
        self.place_root_for_target(node, target)
    }

    pub(super) fn place_root_for_target(
        &mut self,
        node: NodeId,
        target: NameTarget,
    ) -> Result<(PlaceRoot, TypeId), BodyCheckError> {
        Ok(match target {
            NameTarget::Parameter(parameter) => {
                let declaration = self
                    .graph
                    .declarations()
                    .parameters()
                    .get(parameter)
                    .ok_or(BodyCheckInternalError::MissingParameterType(target))?;
                let ty = match declaration.role() {
                    nocter_declarations::ParameterRole::Receiver(capability) => {
                        let capability = match capability {
                            nocter_model::CallableCapability::Readonly => {
                                Some(BorrowCapability::Readonly)
                            }
                            nocter_model::CallableCapability::ReadWrite => {
                                Some(BorrowCapability::ReadWrite)
                            }
                            nocter_model::CallableCapability::Owned => None,
                        };
                        if let Some(capability) = capability {
                            self.types
                                .intern(TypeKind::Borrow {
                                    capability,
                                    referent: declaration.ty(),
                                })
                                .map_err(|_| {
                                    BodyCheckInternalError::UnknownType(declaration.ty())
                                })?
                        } else {
                            declaration.ty()
                        }
                    }
                    nocter_declarations::ParameterRole::Ordinary {
                        variadic: false, ..
                    } => declaration.ty(),
                    nocter_declarations::ParameterRole::Ordinary { variadic: true, .. } => {
                        return Err(self.rule(BodyRule::InvalidLiteralPackUse, node)?);
                    }
                };
                (PlaceRoot::Parameter(parameter), ty)
            }
            NameTarget::Local(local) => (
                PlaceRoot::Local(local),
                self.builder
                    .local_type(local)
                    .ok_or(BodyCheckInternalError::MissingLocalType(local))?,
            ),
            NameTarget::Capture(capture) => (
                PlaceRoot::Capture(capture),
                self.builder
                    .capture_type(capture)
                    .ok_or(BodyCheckInternalError::MissingCaptureType(capture))?,
            ),
            _ => return Err(BodyCheckInternalError::UnsupportedNameTarget(node, target).into()),
        })
    }
}

#[derive(Clone, Copy)]
enum PlaceOperation {
    Field(NodeId),
    Index(NodeId),
}

struct PlaceSyntax {
    root: NodeId,
    operations: Vec<PlaceOperation>,
}

enum PlaceSyntaxError {
    NotPlace(NodeId),
    InvalidSyntax(NodeId),
}

fn collect_postfix_operations(
    tree: &nocter_syntax::SyntaxTree,
    mut node: NodeId,
) -> Result<PlaceSyntax, PlaceSyntaxError> {
    let mut operations = Vec::new();
    loop {
        let kind = tree
            .node(node)
            .map(nocter_syntax::SyntaxNode::kind)
            .ok_or(PlaceSyntaxError::InvalidSyntax(node))?;
        match kind {
            NodeKind::ReferenceExpression => {
                operations.reverse();
                return Ok(PlaceSyntax {
                    root: node,
                    operations,
                });
            }
            kind if is_transparent_expression(kind) => {
                let children = direct_nodes(tree, node);
                if children.len() != 1 {
                    return Err(PlaceSyntaxError::NotPlace(node));
                }
                node = children[0];
            }
            NodeKind::PostfixExpression => {
                let children = direct_nodes(tree, node);
                if children.len() != 2 {
                    return Err(PlaceSyntaxError::InvalidSyntax(node));
                }
                let suffix = tree
                    .node(children[1])
                    .map(nocter_syntax::SyntaxNode::kind)
                    .ok_or(PlaceSyntaxError::InvalidSyntax(children[1]))?;
                operations.push(match suffix {
                    NodeKind::MemberSuffix => PlaceOperation::Field(children[1]),
                    NodeKind::IndexSuffix => PlaceOperation::Index(children[1]),
                    NodeKind::CallSuffix => {
                        if operations.is_empty() {
                            return Err(PlaceSyntaxError::NotPlace(children[1]));
                        }
                        operations.reverse();
                        return Ok(PlaceSyntax {
                            root: node,
                            operations,
                        });
                    }
                    _ => return Err(PlaceSyntaxError::InvalidSyntax(node)),
                });
                node = children[0];
            }
            _ if !operations.is_empty() => {
                operations.reverse();
                return Ok(PlaceSyntax {
                    root: node,
                    operations,
                });
            }
            _ => return Err(PlaceSyntaxError::NotPlace(node)),
        }
    }
}
