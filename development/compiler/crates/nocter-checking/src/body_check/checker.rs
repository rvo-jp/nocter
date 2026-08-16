use std::collections::{HashMap, HashSet};

use nocter_declaration_lowering::CompileUnitInput;
use nocter_declarations::{BodyOwner, DeclarationGraph};
use nocter_model::{
    ArenaBuilder, BodyId, BodyNodeId, BorrowCapability, BuiltinType, LocalBindingId, TypeId,
    TypeKind, TypeStore,
};
use nocter_source_index::{SemanticEntity, SourceIndex, SourceOrigin, SourceRole, SyntaxOrigin};
use nocter_syntax::{
    Keyword, NodeId, NodeKind, Punctuation, SyntaxElement, SyntaxToken, TokenKind,
};

use super::diagnostic::BodyRule;
use super::error::{BodyCheckError, BodyCheckInternalError};
use super::literal::{fits_integer, integer_type, parse_integer};
use crate::checked::{CheckedBodyBuilder, CheckedProgram, CheckedProgramOutput};
use crate::copyability::{Copyability, CopyabilityTable};
use crate::preparation::PreparedCheckingParts;
use crate::syntax::{direct_child, direct_identifier, direct_nodes, token_text};
use crate::{
    BodySource, CheckedBody, CheckedControl, CheckedOperation, CheckedOutcome, ConstantValue,
    ExpectedBase, ExpectedEvidence, ExpectedTypeError, ExpectedTypePlan, NameTarget, OutcomeLayer,
    PlaceAccess, PlaceRoot, PreparedChecking, ResolvedBodyNames, plan_expected_type,
};

struct NodeProjection {
    entity: SemanticEntity,
    origin: SourceOrigin,
}

/// Consumes a fully prepared Phase 3 input and constructs its immutable checked program.
///
/// The current construction slice accepts blocks, scalar literals, copyable parameter/local uses,
/// readonly borrows, bindings, expression statements, body results, and returns. Other valid
/// syntax is reported as an internal incomplete-implementation boundary; no partial checked
/// program escapes.
///
/// # Errors
///
/// Returns one source-backed body rule or an internal consistency/incomplete-implementation error.
pub fn check_prepared_program<'syntax>(
    input: &'syntax CompileUnitInput<'syntax>,
    prepared: PreparedChecking<'syntax>,
) -> Result<CheckedProgramOutput, BodyCheckError> {
    let PreparedCheckingParts {
        graph,
        mut types,
        conformances,
        mut copyabilities,
        body_sources,
        body_names,
        source_index,
    } = prepared.into_parts();
    let mut bodies = ArenaBuilder::<BodyId, CheckedBody>::new();
    let mut projections = Vec::new();

    for (body, _) in graph.declarations().bodies().iter() {
        let source = body_sources
            .get(body)
            .ok_or(BodyCheckInternalError::MissingBodySource(body))?;
        let names = body_names
            .get(body)
            .ok_or(BodyCheckInternalError::MissingBodyNames(body))?;
        if names.body() != body {
            return Err(BodyCheckInternalError::BodyIdentityMismatch(body).into());
        }
        let checked = BodyChecker::new(
            input,
            &graph,
            &mut types,
            &mut copyabilities,
            &source_index,
            source,
            names,
        )?
        .check()?;
        let actual = bodies.insert(checked.body);
        if actual != body {
            return Err(BodyCheckInternalError::NonCanonicalBody(body).into());
        }
        projections.extend(checked.projections);
    }

    let mut source_index = source_index.into_builder();
    for projection in projections {
        source_index
            .insert(projection.entity, SourceRole::Reference, projection.origin)
            .map_err(BodyCheckInternalError::from)?;
    }
    copyabilities
        .complete(&graph, &mut types)
        .map_err(BodyCheckInternalError::Copyability)?;
    Ok(CheckedProgramOutput::new(
        CheckedProgram::new(graph, types, conformances, copyabilities, bodies.finish()),
        source_index.finish(),
    ))
}

struct CheckedBodyOutput {
    body: CheckedBody,
    projections: Vec<NodeProjection>,
}

struct BodyChecker<'input, 'syntax> {
    input: &'input CompileUnitInput<'syntax>,
    graph: &'input DeclarationGraph,
    types: &'input mut TypeStore,
    copyabilities: &'input mut CopyabilityTable,
    source: BodySource<'syntax>,
    names: &'input ResolvedBodyNames,
    builder: CheckedBodyBuilder,
    uses: HashMap<SyntaxOrigin, NameTarget>,
    consumed_uses: HashSet<SyntaxOrigin>,
    local_declarations: HashMap<SyntaxOrigin, LocalBindingId>,
    result_type: TypeId,
    projections: Vec<NodeProjection>,
}

impl<'input, 'syntax> BodyChecker<'input, 'syntax> {
    fn new(
        input: &'input CompileUnitInput<'syntax>,
        graph: &'input DeclarationGraph,
        types: &'input mut TypeStore,
        copyabilities: &'input mut CopyabilityTable,
        source_index: &'input SourceIndex,
        source: BodySource<'syntax>,
        names: &'input ResolvedBodyNames,
    ) -> Result<Self, BodyCheckError> {
        let mut uses = HashMap::new();
        for use_ in names.uses() {
            if uses.insert(use_.origin(), use_.target()).is_some() {
                return Err(BodyCheckInternalError::DuplicateNameUse(use_.origin()).into());
            }
        }
        let mut local_declarations = HashMap::new();
        for (local, _) in names.locals().iter() {
            let entity = SemanticEntity::LocalBinding(source.body(), local);
            let origin = source_index
                .bindings_for(entity)
                .iter()
                .find(|binding| binding.role() == SourceRole::Declaration)
                .map(|binding| binding.origin().syntax())
                .ok_or(BodyCheckInternalError::MissingSource(entity))?;
            if local_declarations.insert(origin, local).is_some() {
                return Err(BodyCheckInternalError::DuplicateLocalDeclaration(origin).into());
            }
        }
        let result_type = body_result_type(graph, types, source)?;
        Ok(Self {
            input,
            graph,
            types,
            copyabilities,
            source,
            names,
            builder: CheckedBodyBuilder::new(names),
            uses,
            consumed_uses: HashSet::new(),
            local_declarations,
            result_type,
            projections: Vec::new(),
        })
    }

    fn check(mut self) -> Result<CheckedBodyOutput, BodyCheckError> {
        let root = self.check_block(self.source.block())?;
        if self.consumed_uses.len() != self.names.uses().len() {
            return Err(BodyCheckInternalError::UnconsumedNameUses(self.source.body()).into());
        }
        Ok(CheckedBodyOutput {
            body: self.builder.finish(root)?,
            projections: self.projections,
        })
    }

    fn check_block(&mut self, block: NodeId) -> Result<BodyNodeId, BodyCheckError> {
        let mut statements = Vec::new();
        let mut result = None;
        let mut reachable = true;
        if let Some(sequence) = direct_child(self.tree(), block, NodeKind::ExecutableSequence) {
            for executable in direct_nodes(self.tree(), sequence) {
                if !reachable {
                    return Err(self.rule(BodyRule::UnreachableCode, executable)?.into());
                }
                match self.kind(executable)? {
                    NodeKind::BodyResult => {
                        let expression = self.required_child(executable, NodeKind::Expression)?;
                        let node = self.check_expression(expression, Some(self.result_type))?;
                        reachable = self.builder.node_type(node)
                            != Some(self.types.builtin(BuiltinType::Never));
                        result = Some(node);
                    }
                    NodeKind::ExpressionStatement => {
                        let expression = self.required_child(executable, NodeKind::Expression)?;
                        let value = self.check_expression(expression, None)?;
                        let ty = self.node_type(value)?;
                        if !matches!(
                            self.types.get(ty),
                            Some(TypeKind::Builtin(BuiltinType::Void | BuiltinType::Never))
                        ) {
                            return Err(self
                                .rule(BodyRule::InvalidStatementValue, executable)?
                                .into());
                        }
                        let discard = self.add_node(
                            executable,
                            ty,
                            CheckedOperation::Control(CheckedControl::Discard(value)),
                        )?;
                        reachable = ty != self.types.builtin(BuiltinType::Never);
                        statements.push(discard);
                    }
                    NodeKind::BindingStatement => {
                        statements.push(self.check_binding(executable)?);
                    }
                    NodeKind::ReturnStatement => {
                        statements.push(self.check_return(executable)?);
                        reachable = false;
                    }
                    kind => {
                        return Err(
                            BodyCheckInternalError::UnsupportedSyntax(executable, kind).into()
                        );
                    }
                }
            }
        }

        if result.is_some() && !reachable {
            return Err(BodyCheckInternalError::InvalidSyntax(block).into());
        }
        let ty = if let Some(result) = result {
            self.node_type(result)?
        } else if !reachable {
            self.types.builtin(BuiltinType::Never)
        } else {
            self.complete_fallthrough(block, &mut result)?
        };
        self.add_node(
            block,
            ty,
            CheckedOperation::Control(CheckedControl::Block {
                statements: statements.into_boxed_slice(),
                result,
            }),
        )
    }

    fn complete_fallthrough(
        &mut self,
        block: NodeId,
        result: &mut Option<BodyNodeId>,
    ) -> Result<TypeId, BodyCheckError> {
        let void = self.types.builtin(BuiltinType::Void);
        if self.result_type == void {
            return Ok(void);
        }
        let completion = self.add_node(block, void, CheckedOperation::Complete)?;
        match self.apply_expected(block, completion, self.result_type) {
            Ok(node) => {
                *result = Some(node);
                Ok(self.node_type(node)?)
            }
            Err(BodyCheckError::Rule(_)) => {
                Err(self.rule(BodyRule::MissingBodyResult, block)?.into())
            }
            Err(error) => Err(error),
        }
    }

    fn check_binding(&mut self, statement: NodeId) -> Result<BodyNodeId, BodyCheckError> {
        if direct_child(self.tree(), statement, NodeKind::TypeAnnotation).is_some() {
            return Err(BodyCheckInternalError::UnsupportedSyntax(
                statement,
                NodeKind::TypeAnnotation,
            )
            .into());
        }
        let target = self.required_child(statement, NodeKind::BindingTarget)?;
        let token = direct_identifier(self.tree(), target)
            .ok_or(BodyCheckInternalError::InvalidSyntax(target))?;
        let initializer = self.required_child(statement, NodeKind::Expression)?;
        let value = self.check_expression(initializer, None)?;
        let ty = self.node_type(value)?;
        if self.token_text(token)? == "_" {
            return self.add_node(
                statement,
                self.types.builtin(BuiltinType::Void),
                CheckedOperation::Control(CheckedControl::Discard(value)),
            );
        }
        let local = self
            .local_declarations
            .get(&SyntaxOrigin::Token(token))
            .copied()
            .ok_or(BodyCheckInternalError::MissingLocalDeclaration(target))?;
        self.builder.define_local(local, ty)?;
        self.add_node(
            statement,
            self.types.builtin(BuiltinType::Void),
            CheckedOperation::Control(CheckedControl::Bind {
                binding: local,
                initializer: value,
            }),
        )
    }

    fn check_return(&mut self, statement: NodeId) -> Result<BodyNodeId, BodyCheckError> {
        let value =
            if let Some(expression) = direct_child(self.tree(), statement, NodeKind::Expression) {
                Some(self.check_expression(expression, Some(self.result_type))?)
            } else if self.result_type == self.types.builtin(BuiltinType::Void) {
                None
            } else {
                let completion = self.add_node(
                    statement,
                    self.types.builtin(BuiltinType::Void),
                    CheckedOperation::Complete,
                )?;
                Some(self.apply_expected(statement, completion, self.result_type)?)
            };
        self.add_node(
            statement,
            self.types.builtin(BuiltinType::Never),
            CheckedOperation::Control(CheckedControl::Return(value)),
        )
    }

    fn check_expression(
        &mut self,
        root: NodeId,
        expected: Option<TypeId>,
    ) -> Result<BodyNodeId, BodyCheckError> {
        let mut current = root;
        loop {
            let kind = self.kind(current)?;
            if is_transparent_expression(kind) {
                let children = direct_nodes(self.tree(), current);
                if children.len() == 1 {
                    current = children[0];
                    continue;
                }
            }
            let value = match kind {
                NodeKind::ScalarLiteral => return self.check_scalar(current, expected),
                NodeKind::ReferenceExpression => self.check_reference(current)?,
                NodeKind::UnaryExpression => self.check_unary(current)?,
                _ => return Err(BodyCheckInternalError::UnsupportedSyntax(current, kind).into()),
            };
            return expected.map_or(Ok(value), |expected| {
                self.apply_expected(current, value, expected)
            });
        }
    }

    fn check_scalar(
        &mut self,
        node: NodeId,
        expected: Option<TypeId>,
    ) -> Result<BodyNodeId, BodyCheckError> {
        let token =
            direct_token(self.tree(), node).ok_or(BodyCheckInternalError::InvalidSyntax(node))?;
        match token.kind() {
            TokenKind::Keyword(Keyword::True | Keyword::False) => {
                let value = token.kind() == TokenKind::Keyword(Keyword::True);
                let checked = self.add_node(
                    node,
                    self.types.builtin(BuiltinType::Bool),
                    CheckedOperation::Constant(ConstantValue::Bool(value)),
                )?;
                expected.map_or(Ok(checked), |expected| {
                    self.apply_expected(node, checked, expected)
                })
            }
            TokenKind::Keyword(Keyword::None) => {
                let Some(expected) = expected else {
                    return Err(self.rule(BodyRule::TypeMismatch, node)?.into());
                };
                self.materialize_plan(
                    node,
                    plan_expected_type(self.types, expected, ExpectedEvidence::Absent)
                        .map_err(|error| self.expected_error(node, error))?,
                    None,
                )
            }
            TokenKind::IntegerLiteral => {
                let ty = integer_type(self.types, expected);
                let Some(value) = parse_integer(self.token_text(token)?)
                    .filter(|value| fits_integer(self.types, ty, *value))
                else {
                    return Err(self.rule(BodyRule::IntegerOutOfRange, node)?.into());
                };
                let checked = self.add_node(
                    node,
                    ty,
                    CheckedOperation::Constant(ConstantValue::Integer(value)),
                )?;
                expected.map_or(Ok(checked), |expected| {
                    self.apply_expected(node, checked, expected)
                })
            }
            _ => {
                Err(BodyCheckInternalError::UnsupportedSyntax(node, NodeKind::ScalarLiteral).into())
            }
        }
    }

    fn check_reference(&mut self, node: NodeId) -> Result<BodyNodeId, BodyCheckError> {
        let (place, ty) = self.named_place(node)?;
        match self
            .copyabilities
            .classify(self.graph, self.types, ty)
            .map_err(BodyCheckInternalError::Copyability)?
        {
            Copyability::Copy => {}
            Copyability::MoveOnly => return Err(self.rule(BodyRule::ImplicitMove, node)?.into()),
        }
        self.add_node(node, ty, CheckedOperation::Copy(place))
    }

    fn check_unary(&mut self, node: NodeId) -> Result<BodyNodeId, BodyCheckError> {
        let token =
            direct_token(self.tree(), node).ok_or(BodyCheckInternalError::InvalidSyntax(node))?;
        if token.kind() != TokenKind::Punctuation(Punctuation::Ampersand) {
            return Err(
                BodyCheckInternalError::UnsupportedSyntax(node, NodeKind::UnaryExpression).into(),
            );
        }
        let operand = single_descendant(self.tree(), node, NodeKind::ReferenceExpression)
            .ok_or(BodyCheckInternalError::InvalidSyntax(node))?;
        let (place, referent) = self.named_place(operand)?;
        let ty = self
            .types
            .intern(TypeKind::Borrow {
                capability: BorrowCapability::Readonly,
                referent,
            })
            .map_err(|_| BodyCheckInternalError::UnknownType(referent))?;
        self.add_node(
            node,
            ty,
            CheckedOperation::Borrow {
                capability: BorrowCapability::Readonly,
                place,
            },
        )
    }

    fn named_place(
        &mut self,
        node: NodeId,
    ) -> Result<(nocter_model::PlaceId, TypeId), BodyCheckError> {
        let token = direct_identifier(self.tree(), node)
            .ok_or(BodyCheckInternalError::InvalidSyntax(node))?;
        let origin = SyntaxOrigin::Token(token);
        let target = self
            .uses
            .get(&origin)
            .copied()
            .ok_or(BodyCheckInternalError::MissingNameUse(node))?;
        self.consumed_uses.insert(origin);
        let (root, ty) = match target {
            NameTarget::Parameter(parameter) => {
                let ty = self
                    .graph
                    .declarations()
                    .parameters()
                    .get(parameter)
                    .map(|parameter| parameter.ty())
                    .ok_or(BodyCheckInternalError::MissingParameterType(target))?;
                (PlaceRoot::Parameter(parameter), ty)
            }
            NameTarget::Local(local) => (
                PlaceRoot::Local(local),
                self.builder
                    .local_type(local)
                    .ok_or(BodyCheckInternalError::MissingLocalType(local))?,
            ),
            _ => return Err(BodyCheckInternalError::UnsupportedNameTarget(node, target).into()),
        };
        Ok((
            self.builder
                .add_place(root, Vec::new().into_boxed_slice(), ty, PlaceAccess::Owned),
            ty,
        ))
    }

    fn apply_expected(
        &mut self,
        node: NodeId,
        value: BodyNodeId,
        expected: TypeId,
    ) -> Result<BodyNodeId, BodyCheckError> {
        let actual = self.node_type(value)?;
        let plan = plan_expected_type(self.types, expected, ExpectedEvidence::Typed(actual))
            .map_err(|error| self.expected_error(node, error))?;
        self.materialize_plan(node, plan, Some(value))
    }

    fn materialize_plan(
        &mut self,
        node: NodeId,
        plan: ExpectedTypePlan,
        payload: Option<BodyNodeId>,
    ) -> Result<BodyNodeId, BodyCheckError> {
        let (base, injections) = plan.into_parts();
        let mut current = match base {
            ExpectedBase::Exact(_) | ExpectedBase::Diverges(_) => {
                payload.ok_or(BodyCheckInternalError::InvalidSyntax(node))?
            }
            ExpectedBase::Absent(ty) => {
                self.add_node(node, ty, CheckedOperation::Outcome(CheckedOutcome::Absent))?
            }
            ExpectedBase::Failure(ty) => self.add_node(
                node,
                ty,
                CheckedOperation::Outcome(CheckedOutcome::Failure(
                    payload.ok_or(BodyCheckInternalError::InvalidSyntax(node))?,
                )),
            )?,
        };
        for layer in injections {
            let payload_type = self.node_type(current)?;
            let ty = self
                .types
                .intern(match layer {
                    OutcomeLayer::Optional => TypeKind::Optional(payload_type),
                    OutcomeLayer::Fallible => TypeKind::Fallible(payload_type),
                })
                .map_err(|_| BodyCheckInternalError::UnknownType(payload_type))?;
            current = self.add_node(
                node,
                ty,
                CheckedOperation::Outcome(CheckedOutcome::Inject {
                    layer,
                    payload: current,
                }),
            )?;
        }
        Ok(current)
    }

    fn expected_error(&self, node: NodeId, error: ExpectedTypeError) -> BodyCheckError {
        match error {
            ExpectedTypeError::Mismatch { .. } => self
                .rule(BodyRule::TypeMismatch, node)
                .map_or_else(BodyCheckError::Internal, BodyCheckError::Rule),
            ExpectedTypeError::UnknownType(ty) => BodyCheckInternalError::UnknownType(ty).into(),
        }
    }

    fn add_node(
        &mut self,
        syntax: NodeId,
        ty: TypeId,
        operation: CheckedOperation,
    ) -> Result<BodyNodeId, BodyCheckError> {
        let node = self.builder.add_node(ty, operation);
        let origin = SourceOrigin::from_node(self.tree(), syntax)
            .map_err(|_| BodyCheckInternalError::InvalidSyntax(syntax))?;
        self.projections.push(NodeProjection {
            entity: SemanticEntity::BodyNode(self.source.body(), node),
            origin,
        });
        Ok(node)
    }

    fn node_type(&self, node: BodyNodeId) -> Result<TypeId, BodyCheckInternalError> {
        self.builder
            .node_type(node)
            .ok_or(BodyCheckInternalError::MissingNode(node))
    }

    fn required_child(
        &self,
        node: NodeId,
        kind: NodeKind,
    ) -> Result<NodeId, BodyCheckInternalError> {
        direct_child(self.tree(), node, kind).ok_or(BodyCheckInternalError::InvalidSyntax(node))
    }

    fn kind(&self, node: NodeId) -> Result<NodeKind, BodyCheckInternalError> {
        self.tree()
            .node(node)
            .map(nocter_syntax::SyntaxNode::kind)
            .ok_or(BodyCheckInternalError::InvalidSyntax(node))
    }

    fn rule(
        &self,
        rule: BodyRule,
        node: NodeId,
    ) -> Result<nocter_diagnostics::SourceDiagnostic, BodyCheckInternalError> {
        let origin = SourceOrigin::from_node(self.tree(), node)
            .map_err(|_| BodyCheckInternalError::InvalidSyntax(node))?;
        Ok(rule.diagnostic(origin))
    }

    fn token_text(&self, token: SyntaxToken) -> Result<&str, BodyCheckInternalError> {
        token_text(self.input.sources(), token)
            .map_err(|_| BodyCheckInternalError::InvalidSyntax(self.source.block()))
    }

    const fn tree(&self) -> &'syntax nocter_syntax::SyntaxTree {
        self.source.syntax()
    }
}

fn body_result_type(
    graph: &DeclarationGraph,
    types: &mut TypeStore,
    source: BodySource<'_>,
) -> Result<TypeId, BodyCheckInternalError> {
    match source.owner() {
        BodyOwner::Callable(callable) => graph
            .declarations()
            .callables()
            .get(callable)
            .map(nocter_declarations::CallableDeclaration::result)
            .ok_or(BodyCheckInternalError::BodyIdentityMismatch(source.body())),
        BodyOwner::Drop(_) => Ok(types.builtin(BuiltinType::Void)),
        BodyOwner::Test(_) => types
            .intern(TypeKind::Fallible(types.builtin(BuiltinType::Void)))
            .map_err(|_| BodyCheckInternalError::UnknownType(types.builtin(BuiltinType::Void))),
    }
}

fn is_transparent_expression(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::Expression
            | NodeKind::LogicalOrExpression
            | NodeKind::LogicalAndExpression
            | NodeKind::EqualityExpression
            | NodeKind::OrderingExpression
            | NodeKind::ShiftExpression
            | NodeKind::AdditiveExpression
            | NodeKind::MultiplicativeExpression
            | NodeKind::ConversionExpression
            | NodeKind::GroupedExpression
    )
}

fn direct_token(tree: &nocter_syntax::SyntaxTree, node: NodeId) -> Option<SyntaxToken> {
    tree.children(node)
        .iter()
        .find_map(|element| match element {
            SyntaxElement::Token(token) => Some(*token),
            SyntaxElement::Node(_) | SyntaxElement::Missing(_) => None,
        })
}

fn single_descendant(
    tree: &nocter_syntax::SyntaxTree,
    root: NodeId,
    expected: NodeKind,
) -> Option<NodeId> {
    let mut current = root;
    loop {
        let children = direct_nodes(tree, current);
        if children.len() != 1 {
            return None;
        }
        current = children[0];
        if tree.node(current)?.kind() == expected {
            return Some(current);
        }
    }
}

#[cfg(test)]
mod tests {
    use nocter_declaration_lowering::lower_compile_unit_declarations;
    use nocter_model::{BuiltinType, TypeKind};
    use nocter_source_index::{SemanticEntity, SourceRole};

    use super::check_prepared_program;
    use crate::test_support::Fixture;
    use crate::{CheckedOperation, CheckedOutcome, OutcomeLayer, prepare_program_checking};

    #[test]
    fn scalar_local_and_body_result_construct_one_closed_checked_body() {
        let fixture = Fixture::new("func answer(): i32 {\n    let value = 42\n    value\n}\n");
        let (input, prelude) = fixture.input(false);
        let lowered = lower_compile_unit_declarations(&input, &prelude).unwrap();
        let (program, source_index) = lowered.into_parts();
        let prepared = prepare_program_checking(&input, program, source_index).unwrap();
        let output = check_prepared_program(&input, prepared).unwrap();
        let (_, body) = output.program().bodies().iter().next().unwrap();

        assert_eq!(body.locals().len(), 1);
        assert_eq!(
            body.locals().iter().next().unwrap().1.ty(),
            output.program().types().builtin(BuiltinType::I32)
        );
        assert!(body.places().iter().next().unwrap().1.is_move_source());
        assert!(matches!(
            body.nodes().get(body.root()).unwrap().operation(),
            CheckedOperation::Control(_)
        ));
        let root_bindings = output.source_index().bindings_for(SemanticEntity::BodyNode(
            output.program().bodies().iter().next().unwrap().0,
            body.root(),
        ));
        assert!(!root_bindings.is_empty());
        assert!(
            root_bindings
                .iter()
                .all(|binding| binding.role() == SourceRole::Reference)
        );
    }

    #[test]
    fn body_result_materializes_recursive_outcome_injection() {
        let fixture = Fixture::new("func answer(): i32?! {\n    42\n}\n");
        let (input, prelude) = fixture.input(false);
        let lowered = lower_compile_unit_declarations(&input, &prelude).unwrap();
        let (program, source_index) = lowered.into_parts();
        let prepared = prepare_program_checking(&input, program, source_index).unwrap();
        let output = check_prepared_program(&input, prepared).unwrap();
        let (_, body) = output.program().bodies().iter().next().unwrap();
        let injections = body
            .nodes()
            .iter()
            .filter_map(|(_, node)| match node.operation() {
                CheckedOperation::Outcome(CheckedOutcome::Inject { layer, .. }) => Some(*layer),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(
            injections,
            vec![OutcomeLayer::Optional, OutcomeLayer::Fallible]
        );
        assert!(matches!(
            output
                .program()
                .types()
                .get(body.nodes().get(body.root()).unwrap().ty()),
            Some(TypeKind::Fallible(_))
        ));
    }

    #[test]
    fn optional_absence_needs_no_synthetic_payload() {
        let fixture = Fixture::new("func answer(): i32? {\n    none\n}\n");
        let (input, prelude) = fixture.input(false);
        let lowered = lower_compile_unit_declarations(&input, &prelude).unwrap();
        let (program, source_index) = lowered.into_parts();
        let prepared = prepare_program_checking(&input, program, source_index).unwrap();
        let output = check_prepared_program(&input, prepared).unwrap();
        let (_, body) = output.program().bodies().iter().next().unwrap();

        assert!(body.nodes().iter().any(|(_, node)| matches!(
            node.operation(),
            CheckedOperation::Outcome(CheckedOutcome::Absent)
        )));
    }

    #[test]
    fn readonly_borrow_uses_the_same_resolved_parameter_place() {
        let fixture = Fixture::new(
            "func observe(value: i32): void {\n    let view = &value\n    return\n}\n",
        );
        let (input, prelude) = fixture.input(false);
        let lowered = lower_compile_unit_declarations(&input, &prelude).unwrap();
        let (program, source_index) = lowered.into_parts();
        let prepared = prepare_program_checking(&input, program, source_index).unwrap();
        let output = check_prepared_program(&input, prepared).unwrap();
        let (_, body) = output.program().bodies().iter().next().unwrap();

        assert!(
            body.nodes()
                .iter()
                .any(|(_, node)| matches!(node.operation(), CheckedOperation::Borrow { .. }))
        );
        assert_eq!(body.places().len(), 1);
    }

    #[test]
    fn copy_struct_specialization_uses_substituted_field_copyability() {
        let fixture = Fixture::new(
            "copy struct Box<T> {\n    value: T\n}\n\
             func duplicate(value: Box<i32>): Box<i32> {\n    value\n}\n",
        );
        let (input, prelude) = fixture.input(false);
        let lowered = lower_compile_unit_declarations(&input, &prelude).unwrap();
        let (program, source_index) = lowered.into_parts();
        let prepared = prepare_program_checking(&input, program, source_index).unwrap();
        let output = check_prepared_program(&input, prepared).unwrap();

        assert!(
            output.program().types().iter().all(|(ty, _)| output
                .program()
                .copyabilities()
                .get(ty)
                .is_some())
        );
        assert!(output.program().bodies().iter().any(|(_, body)| {
            body.nodes()
                .iter()
                .any(|(_, node)| matches!(node.operation(), CheckedOperation::Copy(_)))
        }));
    }

    #[test]
    fn copy_struct_specialization_remains_move_only_for_move_only_argument() {
        let fixture = Fixture::new(
            "struct Owned {\n    value: i32\n}\n\
             copy struct Box<T> {\n    value: T\n}\n\
             func duplicate(value: Box<Owned>): Box<Owned> {\n    value\n}\n",
        );
        let (input, prelude) = fixture.input(false);
        let lowered = lower_compile_unit_declarations(&input, &prelude).unwrap();
        let (program, source_index) = lowered.into_parts();
        let prepared = prepare_program_checking(&input, program, source_index).unwrap();
        let error = check_prepared_program(&input, prepared).unwrap_err();

        assert_eq!(error.source_diagnostic().unwrap().code(), "E0371");
    }

    #[test]
    fn callable_copy_requirement_supplies_the_generic_body_proof() {
        let fixture = Fixture::new("func duplicate<T>(value: T): T where copy T {\n    value\n}\n");
        let (input, prelude) = fixture.input(false);
        let lowered = lower_compile_unit_declarations(&input, &prelude).unwrap();
        let (program, source_index) = lowered.into_parts();
        let prepared = prepare_program_checking(&input, program, source_index).unwrap();

        check_prepared_program(&input, prepared).unwrap();
    }

    #[test]
    fn unconstrained_generic_parameter_is_not_implicitly_copied() {
        let fixture = Fixture::new("func duplicate<T>(value: T): T {\n    value\n}\n");
        let (input, prelude) = fixture.input(false);
        let lowered = lower_compile_unit_declarations(&input, &prelude).unwrap();
        let (program, source_index) = lowered.into_parts();
        let prepared = prepare_program_checking(&input, program, source_index).unwrap();
        let error = check_prepared_program(&input, prepared).unwrap_err();

        assert_eq!(error.source_diagnostic().unwrap().code(), "E0371");
    }

    #[test]
    fn payloadless_enum_is_copyable_without_a_marker() {
        let fixture = Fixture::new(
            "enum Choice {\n    yes\n    no\n}\n\
             func duplicate(value: Choice): Choice {\n    value\n}\n",
        );
        let (input, prelude) = fixture.input(false);
        let lowered = lower_compile_unit_declarations(&input, &prelude).unwrap();
        let (program, source_index) = lowered.into_parts();
        let prepared = prepare_program_checking(&input, program, source_index).unwrap();

        check_prepared_program(&input, prepared).unwrap();
    }

    #[test]
    fn readonly_and_readwrite_borrows_have_distinct_copyability() {
        let readonly =
            Fixture::new("func duplicate(value: &i32): &i32 from value {\n    value\n}\n");
        let (input, prelude) = readonly.input(false);
        let lowered = lower_compile_unit_declarations(&input, &prelude).unwrap();
        let (program, source_index) = lowered.into_parts();
        let prepared = prepare_program_checking(&input, program, source_index).unwrap();
        check_prepared_program(&input, prepared).unwrap();

        let readwrite =
            Fixture::new("func duplicate(value: &+i32): &+i32 from value {\n    value\n}\n");
        let (input, prelude) = readwrite.input(false);
        let lowered = lower_compile_unit_declarations(&input, &prelude).unwrap();
        let (program, source_index) = lowered.into_parts();
        let prepared = prepare_program_checking(&input, program, source_index).unwrap();
        let error = check_prepared_program(&input, prepared).unwrap_err();

        assert_eq!(error.source_diagnostic().unwrap().code(), "E0371");
    }

    #[test]
    fn reachable_nonvoid_fallthrough_has_one_body_rule() {
        let fixture = Fixture::new("func missing(): i32 {}\n");
        let (input, prelude) = fixture.input(false);
        let lowered = lower_compile_unit_declarations(&input, &prelude).unwrap();
        let (program, source_index) = lowered.into_parts();
        let prepared = prepare_program_checking(&input, program, source_index).unwrap();
        let error = check_prepared_program(&input, prepared).unwrap_err();

        assert_eq!(error.source_diagnostic().unwrap().code(), "E0373");
    }

    #[test]
    fn nonfinal_value_expression_is_not_implicitly_discarded() {
        let fixture = Fixture::new("func invalid(): void {\n    42\n    return\n}\n");
        let (input, prelude) = fixture.input(false);
        let lowered = lower_compile_unit_declarations(&input, &prelude).unwrap();
        let (program, source_index) = lowered.into_parts();
        let prepared = prepare_program_checking(&input, program, source_index).unwrap();
        let error = check_prepared_program(&input, prepared).unwrap_err();

        assert_eq!(error.source_diagnostic().unwrap().code(), "E0372");
    }
}
