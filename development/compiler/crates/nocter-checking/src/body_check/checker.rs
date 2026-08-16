use std::collections::{HashMap, HashSet};

use nocter_declaration_lowering::CompileUnitInput;
use nocter_declarations::DeclarationGraph;
use nocter_diagnostics::DiagnosticNote;
use nocter_model::{
    ArenaBuilder, BodyId, BodyNodeId, BorrowCapability, BuiltinType, LocalBindingId, NominalTypeId,
    PlaceId, TypeId, TypeKind, TypeStore,
};
use nocter_source_index::{SemanticEntity, SourceIndex, SourceOrigin, SourceRole, SyntaxOrigin};
use nocter_syntax::{
    Keyword, NodeId, NodeKind, Punctuation, SyntaxElement, SyntaxToken, TokenKind,
};

use super::assumptions::body_assumptions;
use super::context::{BodyProgramFacts, body_result_type};
use super::diagnostic::BodyRule;
use super::error::{BodyCheckError, BodyCheckInternalError};
use super::literal::{fits_integer, integer_type, parse_integer};
use super::ownership::analyze_body_ownership;
use crate::checked::{CheckedBodyBuilder, CheckedProgram, CheckedProgramOutput};
use crate::copyability::{Copyability, CopyabilityTable};
use crate::preparation::PreparedCheckingParts;
use crate::syntax::{
    direct_child, direct_identifier, direct_nodes, direct_token, identifier_tokens,
    is_transparent_expression, token_text,
};
use crate::{
    BodySource, CheckedBody, CheckedControl, CheckedOperation, CheckedOutcome, ConstantValue,
    DropTable, ExpectedBase, ExpectedEvidence, ExpectedTypeError, ExpectedTypePlan, NameTarget,
    OutcomeLayer, PlaceAccess, PlaceProjection, PreparedChecking, ResolvedBodyNames,
    plan_expected_type,
};

mod arithmetic;
mod assignment;
mod loops;
mod place;
use loops::LoopConstruction;

struct NodeProjection {
    entity: SemanticEntity,
    origin: SourceOrigin,
}

struct ResolvedPlace {
    id: PlaceId,
    ty: TypeId,
    access: PlaceAccess,
    partial_parents: Box<[NominalTypeId]>,
}

struct CheckedExecutable {
    node: BodyNodeId,
    result: bool,
    reaches_next: bool,
}

#[derive(Clone, Copy)]
enum BlockExpectation {
    Callable,
    Value(Option<TypeId>),
}

/// Consumes a fully prepared Phase 3 input and constructs its immutable checked program.
///
/// The current construction slice accepts blocks, scalar literals, named places and field moves,
/// readonly/readwrite borrows, field and built-in index places, bindings, conditionals,
/// while/infinite/integer-range loops, loop control, expression statements, body results, and
/// returns. Other valid syntax is reported as an internal incomplete-implementation boundary; no
/// partial checked program escapes.
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
        instance_operations,
        mut copyabilities,
        drops,
        body_sources,
        body_names,
        source_index,
    } = prepared.into_parts();
    let mut bodies = ArenaBuilder::<BodyId, CheckedBody>::new();
    let mut projections = Vec::new();
    let facts = BodyProgramFacts::new(
        &graph,
        &drops,
        &conformances,
        &instance_operations,
        &source_index,
    );

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
        let checked =
            BodyChecker::new(input, facts, &mut types, &mut copyabilities, source, names)?
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
        CheckedProgram::new(
            graph,
            types,
            conformances,
            instance_operations,
            copyabilities,
            drops,
            bodies.finish(),
        ),
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
    drops: &'input DropTable,
    conformances: &'input crate::ConformanceTable,
    instance_operations: &'input crate::InstanceOperationTable,
    source_index: &'input SourceIndex,
    source: BodySource<'syntax>,
    names: &'input ResolvedBodyNames,
    builder: CheckedBodyBuilder,
    uses: HashMap<SyntaxOrigin, NameTarget>,
    consumed_uses: HashSet<SyntaxOrigin>,
    local_declarations: HashMap<SyntaxOrigin, LocalBindingId>,
    result_type: TypeId,
    projections: Vec<NodeProjection>,
    node_origins: HashMap<BodyNodeId, SourceOrigin>,
    loops: Vec<LoopConstruction>,
    flow_reachable: bool,
    assumptions: Vec<crate::CheckedRequirement>,
}

impl<'input, 'syntax> BodyChecker<'input, 'syntax> {
    fn new(
        input: &'input CompileUnitInput<'syntax>,
        facts: BodyProgramFacts<'input>,
        types: &'input mut TypeStore,
        copyabilities: &'input mut CopyabilityTable,
        source: BodySource<'syntax>,
        names: &'input ResolvedBodyNames,
    ) -> Result<Self, BodyCheckError> {
        let graph = facts.graph();
        let drops = facts.drops();
        let conformances = facts.conformances();
        let instance_operations = facts.instance_operations();
        let source_index = facts.source_index();
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
        let assumptions = body_assumptions(graph, types, conformances, instance_operations, source)
            .map_err(BodyCheckInternalError::BodyAssumptions)?;
        Ok(Self {
            input,
            graph,
            types,
            copyabilities,
            drops,
            conformances,
            instance_operations,
            source_index,
            source,
            names,
            builder: CheckedBodyBuilder::new(names),
            uses,
            consumed_uses: HashSet::new(),
            local_declarations,
            result_type,
            projections: Vec::new(),
            node_origins: HashMap::new(),
            loops: Vec::new(),
            flow_reachable: true,
            assumptions,
        })
    }

    fn check(mut self) -> Result<CheckedBodyOutput, BodyCheckError> {
        let root = self.check_block(self.source.block(), BlockExpectation::Callable)?;
        if self.consumed_uses.len() != self.names.uses().len() {
            return Err(BodyCheckInternalError::UnconsumedNameUses(self.source.body()).into());
        }
        let body = self.builder.finish(root)?;
        let cleanups = analyze_body_ownership(
            self.graph,
            self.types,
            self.copyabilities,
            self.drops,
            self.source,
            &body,
            &self.node_origins,
        )?;
        let body = body.with_cleanups(cleanups)?;
        Ok(CheckedBodyOutput {
            body,
            projections: self.projections,
        })
    }

    fn check_block(
        &mut self,
        block: NodeId,
        expectation: BlockExpectation,
    ) -> Result<BodyNodeId, BodyCheckError> {
        let mut statements = Vec::new();
        let mut result = None;
        let mut reachable = true;
        if let Some(sequence) = direct_child(self.tree(), block, NodeKind::ExecutableSequence) {
            for executable in direct_nodes(self.tree(), sequence) {
                if !reachable {
                    statements.push(self.check_unreachable_executable(executable)?);
                    continue;
                }
                let expected_result = match expectation {
                    BlockExpectation::Callable => Some(self.result_type),
                    BlockExpectation::Value(expected) => expected,
                };
                let checked = self.check_executable(executable, expected_result)?;
                reachable = checked.reaches_next;
                if checked.result {
                    result = Some(checked.node);
                } else {
                    statements.push(checked.node);
                }
            }
        }

        let ty = if let Some(result) = result {
            self.node_type(result)?
        } else if !reachable {
            self.types.builtin(BuiltinType::Never)
        } else {
            match expectation {
                BlockExpectation::Callable => self.complete_fallthrough(block, &mut result)?,
                BlockExpectation::Value(expected) => {
                    self.complete_value_fallthrough(block, expected, &mut result)?
                }
            }
        };
        self.add_node(
            block,
            ty,
            CheckedOperation::Control(CheckedControl::Block {
                scope: self
                    .names
                    .block_scope(block)
                    .ok_or(BodyCheckInternalError::MissingBlockScope(block))?,
                statements: statements.into_boxed_slice(),
                result,
            }),
        )
    }

    fn complete_value_fallthrough(
        &mut self,
        block: NodeId,
        expected: Option<TypeId>,
        result: &mut Option<BodyNodeId>,
    ) -> Result<TypeId, BodyCheckError> {
        let void = self.types.builtin(BuiltinType::Void);
        let Some(expected) = expected else {
            return Ok(void);
        };
        let completion = self.add_node(block, void, CheckedOperation::Complete)?;
        let value = self.apply_expected(block, completion, expected)?;
        *result = Some(value);
        self.node_type(value).map_err(Into::into)
    }

    fn check_executable(
        &mut self,
        executable: NodeId,
        expected_result: Option<TypeId>,
    ) -> Result<CheckedExecutable, BodyCheckError> {
        let never = self.types.builtin(BuiltinType::Never);
        match self.kind(executable)? {
            NodeKind::BodyResult => {
                let expression = self.required_child(executable, NodeKind::Expression)?;
                let node = self.check_expression(expression, expected_result)?;
                Ok(CheckedExecutable {
                    node,
                    result: true,
                    reaches_next: self.node_type(node)? != never,
                })
            }
            NodeKind::ExpressionStatement => {
                let expression = self.required_child(executable, NodeKind::Expression)?;
                let value = self.check_expression(expression, None)?;
                let ty = self.node_type(value)?;
                if !matches!(
                    self.types.get(ty),
                    Some(TypeKind::Builtin(BuiltinType::Void | BuiltinType::Never))
                ) {
                    return Err(self.rule(BodyRule::InvalidStatementValue, executable)?);
                }
                Ok(CheckedExecutable {
                    node: self.add_node(
                        executable,
                        ty,
                        CheckedOperation::Control(CheckedControl::Discard(value)),
                    )?,
                    result: false,
                    reaches_next: ty != never,
                })
            }
            NodeKind::BindingStatement => Ok(CheckedExecutable {
                node: self.check_binding(executable)?,
                result: false,
                reaches_next: true,
            }),
            NodeKind::AssignmentStatement => {
                let node = self.check_assignment(executable)?;
                Ok(CheckedExecutable {
                    node,
                    result: false,
                    reaches_next: self.node_type(node)? != never,
                })
            }
            NodeKind::ReturnStatement => Ok(CheckedExecutable {
                node: self.check_return(executable)?,
                result: false,
                reaches_next: false,
            }),
            NodeKind::WhileStatement | NodeKind::LoopStatement | NodeKind::ForStatement => {
                let node = self.check_loop(executable)?;
                Ok(CheckedExecutable {
                    node,
                    result: false,
                    reaches_next: self.node_type(node)? != never,
                })
            }
            NodeKind::BreakStatement => Ok(CheckedExecutable {
                node: self.check_loop_control(executable, true)?,
                result: false,
                reaches_next: false,
            }),
            NodeKind::ContinueStatement => Ok(CheckedExecutable {
                node: self.check_loop_control(executable, false)?,
                result: false,
                reaches_next: false,
            }),
            NodeKind::DropStatement => Ok(CheckedExecutable {
                node: self.check_drop(executable)?,
                result: false,
                reaches_next: true,
            }),
            kind => Err(BodyCheckInternalError::UnsupportedSyntax(executable, kind).into()),
        }
    }

    fn check_unreachable_executable(
        &mut self,
        executable: NodeId,
    ) -> Result<BodyNodeId, BodyCheckError> {
        let previous = self.flow_reachable;
        self.flow_reachable = false;
        let checked = self.check_executable(executable, None);
        self.flow_reachable = previous;
        let checked = checked?;
        self.add_node(
            executable,
            self.types.builtin(BuiltinType::Never),
            CheckedOperation::Control(CheckedControl::Unreachable(checked.node)),
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
            Err(BodyCheckError::Rule { .. }) => Err(self.rule(BodyRule::MissingBodyResult, block)?),
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

    fn check_drop(&mut self, statement: NodeId) -> Result<BodyNodeId, BodyCheckError> {
        let token = identifier_tokens(self.tree(), statement)
            .last()
            .copied()
            .ok_or(BodyCheckInternalError::InvalidSyntax(statement))?;
        let (root, ty) = self.place_root(statement, token)?;
        if matches!(self.types.get(ty), Some(TypeKind::Borrow { .. }))
            || self
                .copyabilities
                .classify(self.graph, self.types, ty)
                .map_err(BodyCheckInternalError::Copyability)?
                == Copyability::Copy
        {
            return Err(self.rule(BodyRule::InvalidExplicitDrop, statement)?);
        }
        let place = self.builder.add_place(
            root,
            Vec::<PlaceProjection>::new(),
            ty,
            PlaceAccess::Owned,
            false,
        );
        self.add_node(
            statement,
            self.types.builtin(BuiltinType::Void),
            CheckedOperation::Control(CheckedControl::Drop(place)),
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
                NodeKind::IfExpression => return self.check_if(current, expected),
                NodeKind::AdditiveExpression | NodeKind::MultiplicativeExpression => {
                    return self.check_arithmetic(current, expected);
                }
                NodeKind::PostfixExpression => self.check_postfix_reference(current)?,
                NodeKind::ReferenceExpression => self.check_reference(current)?,
                NodeKind::MoveExpression => self.check_move(current)?,
                NodeKind::UnaryExpression => self.check_unary(current)?,
                _ => return Err(BodyCheckInternalError::UnsupportedSyntax(current, kind).into()),
            };
            return expected.map_or(Ok(value), |expected| {
                self.apply_expected(current, value, expected)
            });
        }
    }

    fn check_if(
        &mut self,
        node: NodeId,
        expected: Option<TypeId>,
    ) -> Result<BodyNodeId, BodyCheckError> {
        let condition = self.required_child(node, NodeKind::IfCondition)?;
        if direct_child(self.tree(), condition, NodeKind::EnumPattern).is_some() {
            return Err(BodyCheckInternalError::UnsupportedSyntax(
                condition,
                NodeKind::EnumPattern,
            )
            .into());
        }
        let condition_expression = self.required_child(condition, NodeKind::Expression)?;
        let condition = self.check_expression(
            condition_expression,
            Some(self.types.builtin(BuiltinType::Bool)),
        )?;
        let then_syntax = self.required_child(node, NodeKind::Block)?;
        let else_syntax = direct_child(self.tree(), node, NodeKind::ElseClause);
        let then_expectation = if else_syntax.is_some() {
            BlockExpectation::Value(expected)
        } else {
            BlockExpectation::Value(Some(self.types.builtin(BuiltinType::Void)))
        };
        let then_branch = self.check_block(then_syntax, then_expectation)?;
        let then_type = self.node_type(then_branch)?;
        let (else_branch, else_type) = if let Some(else_clause) = else_syntax {
            let inferred = expected
                .or((then_type != self.types.builtin(BuiltinType::Never)).then_some(then_type));
            let branch =
                if let Some(block) = direct_child(self.tree(), else_clause, NodeKind::Block) {
                    self.check_block(block, BlockExpectation::Value(inferred))?
                } else if let Some(if_expression) =
                    direct_child(self.tree(), else_clause, NodeKind::IfExpression)
                {
                    self.check_if(if_expression, inferred)?
                } else {
                    return Err(BodyCheckInternalError::InvalidSyntax(else_clause).into());
                };
            (Some(branch), Some(self.node_type(branch)?))
        } else {
            (None, None)
        };

        let never = self.types.builtin(BuiltinType::Never);
        let ty = match else_type {
            None => self.types.builtin(BuiltinType::Void),
            Some(else_type) if then_type == never => else_type,
            Some(_) => then_type,
        };
        let checked = self.add_node(
            node,
            ty,
            CheckedOperation::Control(CheckedControl::If {
                condition,
                then_branch,
                else_branch,
            }),
        )?;
        expected.map_or(Ok(checked), |expected| {
            self.apply_expected(node, checked, expected)
        })
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
                    return Err(self.rule(BodyRule::TypeMismatch, node)?);
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
                    return Err(self.rule(BodyRule::IntegerOutOfRange, node)?);
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
        let place = self.named_place(node)?;
        self.add_node(node, place.ty, CheckedOperation::Copy(place.id))
    }

    fn check_postfix_reference(&mut self, node: NodeId) -> Result<BodyNodeId, BodyCheckError> {
        let place = self.postfix_place(node, BorrowCapability::Readonly)?;
        self.add_node(node, place.ty, CheckedOperation::Copy(place.id))
    }

    fn check_move(&mut self, node: NodeId) -> Result<BodyNodeId, BodyCheckError> {
        if self.tree().children(node).iter().any(|element| {
            matches!(
                element,
                SyntaxElement::Token(token)
                    if matches!(
                        token.kind(),
                        TokenKind::Punctuation(Punctuation::Question | Punctuation::Bang)
                    )
            )
        }) {
            return Err(
                BodyCheckInternalError::UnsupportedSyntax(node, NodeKind::MoveExpression).into(),
            );
        }
        let operand = self.required_child(node, NodeKind::NamedPlace)?;
        let place = self.named_place(operand)?;
        match self
            .copyabilities
            .classify(self.graph, self.types, place.ty)
            .map_err(BodyCheckInternalError::Copyability)?
        {
            Copyability::Copy => {
                return Err(self.rule(BodyRule::MoveCopyValue, node)?);
            }
            Copyability::MoveOnly => {}
        }
        if place.access != PlaceAccess::Owned
            || matches!(self.types.get(place.ty), Some(TypeKind::Borrow { .. }))
        {
            return Err(self.rule(BodyRule::InvalidMoveSource, node)?);
        }
        for parent in place.partial_parents.iter().rev() {
            if let Some(drop) = self.drops.get(*parent) {
                return Err(self.partial_move_drop(node, drop)?);
            }
        }
        self.add_node(node, place.ty, CheckedOperation::Move(place.id))
    }

    fn check_unary(&mut self, node: NodeId) -> Result<BodyNodeId, BodyCheckError> {
        let token =
            direct_token(self.tree(), node).ok_or(BodyCheckInternalError::InvalidSyntax(node))?;
        let capability = match token.kind() {
            TokenKind::Punctuation(Punctuation::Ampersand) => BorrowCapability::Readonly,
            TokenKind::Punctuation(Punctuation::ReadWrite) => BorrowCapability::ReadWrite,
            _ => {
                return Err(BodyCheckInternalError::UnsupportedSyntax(
                    node,
                    NodeKind::UnaryExpression,
                )
                .into());
            }
        };
        let operands = direct_nodes(self.tree(), node);
        if operands.len() != 1 {
            return Err(
                BodyCheckInternalError::UnsupportedSyntax(node, NodeKind::UnaryExpression).into(),
            );
        }
        let place = self.postfix_place(operands[0], capability)?;
        if capability == BorrowCapability::ReadWrite && !self.is_writable_place(place.id)? {
            return Err(self.rule(BodyRule::InvalidReadWriteBorrow, operands[0])?);
        }
        let ty = self
            .types
            .intern(TypeKind::Borrow {
                capability,
                referent: place.ty,
            })
            .map_err(|_| BodyCheckInternalError::UnknownType(place.ty))?;
        self.add_node(
            node,
            ty,
            CheckedOperation::Borrow {
                capability,
                place: place.id,
            },
        )
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
                .unwrap_or_else(BodyCheckError::Internal),
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
        if self.node_origins.insert(node, origin).is_some() {
            return Err(BodyCheckInternalError::DuplicateNodeOrigin(node).into());
        }
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

    fn rule(&self, rule: BodyRule, node: NodeId) -> Result<BodyCheckError, BodyCheckInternalError> {
        let origin = SourceOrigin::from_node(self.tree(), node)
            .map_err(|_| BodyCheckInternalError::InvalidSyntax(node))?;
        Ok(BodyCheckError::from_rule(rule, rule.diagnostic(origin)))
    }

    fn token_rule(
        &self,
        rule: BodyRule,
        token: SyntaxToken,
    ) -> Result<BodyCheckError, BodyCheckInternalError> {
        let origin = SourceOrigin::from_token(self.tree(), token)
            .map_err(|_| BodyCheckInternalError::InvalidSyntax(self.source.block()))?;
        Ok(BodyCheckError::from_rule(rule, rule.diagnostic(origin)))
    }

    fn partial_move_drop(
        &self,
        node: NodeId,
        drop: nocter_model::DropId,
    ) -> Result<BodyCheckError, BodyCheckInternalError> {
        let primary = SourceOrigin::from_node(self.tree(), node)
            .map_err(|_| BodyCheckInternalError::InvalidSyntax(node))?;
        let entity = SemanticEntity::Drop(drop);
        let related = self
            .source_index
            .bindings_for(entity)
            .iter()
            .find(|binding| {
                matches!(
                    binding.role(),
                    SourceRole::Declaration | SourceRole::Implementation
                )
            })
            .map(|binding| binding.origin())
            .ok_or(BodyCheckInternalError::MissingSource(entity))?;
        Ok(BodyCheckError::from_rule(
            BodyRule::PartialMoveThroughDrop,
            BodyRule::PartialMoveThroughDrop.diagnostic_with_notes(
                primary,
                [DiagnosticNote::new(
                    "the enclosing struct's drop declaration is here",
                    related,
                )],
            ),
        ))
    }

    fn token_text(&self, token: SyntaxToken) -> Result<&str, BodyCheckInternalError> {
        token_text(self.input.sources(), token)
            .map_err(|_| BodyCheckInternalError::InvalidSyntax(self.source.block()))
    }

    const fn tree(&self) -> &'syntax nocter_syntax::SyntaxTree {
        self.source.syntax()
    }
}
