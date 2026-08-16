use nocter_model::{BorrowCapability, BuiltinType, TypeId, TypeKind};
use nocter_source_index::{SemanticEntity, SourceOrigin, SyntaxOrigin};
use nocter_syntax::{NodeId, NodeKind, SyntaxToken};

use super::{BodyChecker, NodeProjection, ResolvedPlace};
use crate::body_check::diagnostic::BodyRule;
use crate::body_check::error::{BodyCheckError, BodyCheckInternalError};
use crate::field_selection::{FieldSelectionError, select_field};
use crate::instance_operations::{
    IndexOperationCandidate, InstanceOperationSelector, retain_direct_candidates,
};
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
    partial_parents: Vec<nocter_model::NominalTypeId>,
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
        let token = direct_identifier(self.tree(), syntax.root)
            .ok_or(BodyCheckInternalError::InvalidSyntax(syntax.root))?;
        let mut draft = self.start_place(syntax.root, token)?;
        for operation in syntax.operations {
            match operation {
                PlaceOperation::Field(suffix) => {
                    let field = direct_identifier(self.tree(), suffix)
                        .ok_or(BodyCheckInternalError::InvalidSyntax(suffix))?;
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

    fn start_place(
        &mut self,
        node: NodeId,
        token: SyntaxToken,
    ) -> Result<PlaceDraft, BodyCheckError> {
        let (root, ty) = self.place_root(node, token)?;
        let writable = match root {
            PlaceRoot::Local(local) => self
                .names
                .locals()
                .get(local)
                .is_some_and(|local| local.kind() == LocalBindingKind::Mutable),
            PlaceRoot::Parameter(_) | PlaceRoot::Capture(_) => false,
        };
        Ok(PlaceDraft {
            root,
            ty,
            access: PlaceAccess::Owned,
            writable,
            projections: Vec::new(),
            partial_parents: Vec::new(),
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
                | FieldSelectionError::Substitution(_),
            ) => return Err(BodyCheckInternalError::FieldSelection.into()),
        };
        if draft.access == PlaceAccess::Owned {
            draft.partial_parents.push(selected.owner());
        }
        let origin = SourceOrigin::from_token(self.tree(), field_token)
            .map_err(|_| BodyCheckInternalError::InvalidSyntax(node))?;
        self.projections.push(NodeProjection {
            entity: SemanticEntity::Field(selected.field()),
            origin,
        });
        draft
            .projections
            .push(PlaceProjection::Field(selected.field()));
        draft.ty = selected.ty();
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
            return Ok(());
        }
        let receiver_writable = draft.writable;
        let mut candidates = {
            let mut selector = InstanceOperationSelector::new(
                self.graph,
                self.types,
                self.conformances,
                self.copyabilities,
                self.instance_operations,
                &self.assumptions,
                self.source.module(),
            );
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
                }
                Some(_) => return Ok(ty),
                None => return Err(BodyCheckInternalError::UnknownType(ty).into()),
            }
        }
    }

    fn finish_place(&mut self, draft: PlaceDraft) -> ResolvedPlace {
        ResolvedPlace {
            id: self.builder.add_place(
                draft.root,
                draft.projections,
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
                    nocter_declarations::ParameterRole::Ordinary { .. } => declaration.ty(),
                };
                (PlaceRoot::Parameter(parameter), ty)
            }
            NameTarget::Local(local) => (
                PlaceRoot::Local(local),
                self.builder
                    .local_type(local)
                    .ok_or(BodyCheckInternalError::MissingLocalType(local))?,
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
                        return Err(PlaceSyntaxError::NotPlace(children[1]));
                    }
                    _ => return Err(PlaceSyntaxError::InvalidSyntax(node)),
                });
                node = children[0];
            }
            _ => return Err(PlaceSyntaxError::NotPlace(node)),
        }
    }
}
