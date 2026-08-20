use std::collections::{HashMap, HashSet};

use nocter_declaration_lowering::CompileUnitInput;
use nocter_declarations::DeclarationGraph;
use nocter_diagnostics::DiagnosticNote;
use nocter_model::{
    Arena, ArenaBuilder, BodyId, BodyNodeId, BorrowCapability, BuiltinType, CaptureId,
    LocalBindingId, NominalTypeId, PlaceId, TypeId, TypeKind, TypeStore,
};
use nocter_source_index::{SemanticEntity, SourceIndex, SourceOrigin, SourceRole, SyntaxOrigin};
use nocter_syntax::{
    Keyword, NodeId, NodeKind, Punctuation, SyntaxElement, SyntaxToken, TokenKind,
};

use super::assumptions::body_assumptions;
use super::context::{BodyProgramFacts, body_generic_domain, body_result_type};
use super::diagnostic::BodyRule;
use super::error::{BodyCheckError, BodyCheckInternalError};
use super::literal::{fits_integer, integer_type, parse_integer};
use super::ownership::{OwnershipBodyInput, analyze_body_ownership};
use crate::checked::{
    CheckedBodyBuilder, CheckedProgram, CheckedProgramAuthorities, CheckedProgramOutput,
    ClosureTableBuilder,
};
use crate::copyability::{CopyProofs, Copyability, CopyabilityTable};
use crate::instance_operations::{InstanceOperationSelector, InstanceSelectionContext};
use crate::loans::{LoanBodyInput, analyze_program_loans};
use crate::preparation::PreparedCheckingParts;
use crate::provenance::{ProvenanceBodyInput, analyze_program_provenance};
use crate::syntax::{
    direct_child, direct_identifier, direct_nodes, direct_token, identifier_tokens,
    is_transparent_expression, token_text,
};
use crate::{
    BodySource, BodySourceCatalog, CheckedBody, CheckedControl, CheckedOperation, ConstantValue,
    DropTable, ExpectedEvidence, NameTarget, PlaceAccess, PlaceProjection, PreparedChecking,
    ResolvedBodyNames, plan_expected_type,
};

mod aggregates;
mod allocation;
mod arithmetic;
mod assignment;
mod call_planning;
mod callable_values;
mod calls;
mod closure_results;
mod closures;
mod construction_planning;
mod constructions;
mod conversions;
mod expected;
mod interpolation;
mod iterations;
mod literal_pack;
mod loops;
mod methods;
mod opaque_witness;
mod operators;
mod outcomes;
mod patterns;
mod place;
mod readonly_operands;
mod regions;
mod type_uses;
mod typed_literals;
mod value_planning;
use loops::LoopConstruction;
use opaque_witness::OpaqueResultState;

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

struct CheckedIfBranches {
    then_branch: BodyNodeId,
    else_branch: Option<BodyNodeId>,
    ty: TypeId,
}

#[derive(Clone, Copy)]
enum BlockExpectation {
    Callable,
    Value(Option<TypeId>),
}

/// Consumes a fully prepared Phase 3 input and constructs its immutable checked program.
///
/// The current construction slice accepts blocks, scalar literals, named places and field moves,
/// readonly/readwrite borrows, built-in and selected index places, primitive and selected
/// comparisons, primitive operators, direct static calls, inferred and annotated bindings,
/// conditionals, short-circuit logic, while/infinite/range/collection loops, loop control,
/// expression statements, body results, and returns. Other valid syntax is reported as an internal
/// incomplete-implementation boundary; no partial checked program escapes.
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
        construction_surfaces,
        instance_operations,
        mut copyabilities,
        drops,
        standard_semantics,
        body_sources,
        body_names,
        source_index,
    } = prepared.into_parts();
    let mut closures = ClosureTableBuilder::new();
    let facts = BodyProgramFacts::new(
        &graph,
        &drops,
        &conformances,
        &construction_surfaces,
        &instance_operations,
        &standard_semantics,
        &source_index,
    );

    let CheckedBodiesOutput {
        bodies: mut checked_bodies,
        projections,
        opaque_witnesses,
    } = check_declared_bodies(
        input,
        facts,
        &mut types,
        &mut copyabilities,
        &mut closures,
        &body_sources,
        &body_names,
    )?;

    let closures = closures.finish()?;
    let opaque_witnesses = crate::OpaqueWitnessTable::build(&graph, opaque_witnesses)
        .map_err(|_| BodyCheckInternalError::OpaqueWitnessPlanning)?;
    attach_body_cleanups(
        facts,
        &mut types,
        &mut copyabilities,
        &closures,
        &body_sources,
        &mut checked_bodies,
    )?;

    let (provenance, loans) = analyze_checked_body_relations(
        &graph,
        &types,
        &drops,
        &conformances,
        &closures,
        &body_sources,
        &checked_bodies,
    )?;

    let mut bodies = ArenaBuilder::<BodyId, CheckedBody>::new();
    for (body, checked) in checked_bodies {
        let actual = bodies.insert(checked.body);
        if actual != body {
            return Err(BodyCheckInternalError::NonCanonicalBody(body).into());
        }
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
            CheckedProgramAuthorities {
                conformances,
                construction_surfaces,
                instance_operations,
                copyabilities,
                drops,
                standard_semantics,
                provenance,
                loans,
                closures,
                opaque_witnesses,
            },
            bodies.finish(),
        ),
        source_index.finish(),
    ))
}

fn attach_body_cleanups(
    facts: BodyProgramFacts<'_>,
    types: &mut TypeStore,
    copyabilities: &mut CopyabilityTable,
    closures: &crate::ClosureTable,
    body_sources: &BodySourceCatalog<'_>,
    checked_bodies: &mut [(BodyId, CheckedBodyOutput)],
) -> Result<(), BodyCheckError> {
    for (body, checked) in checked_bodies {
        let source = body_sources
            .get(*body)
            .ok_or(BodyCheckInternalError::MissingBodySource(*body))?;
        let cleanups = analyze_body_ownership(
            facts.graph(),
            types,
            copyabilities,
            facts.drops(),
            closures,
            OwnershipBodyInput::new(
                source,
                &checked.body,
                &checked.node_origins,
                &checked.copy_proofs,
            ),
        )?;
        checked.body.attach_cleanups(cleanups)?;
    }
    Ok(())
}

fn check_declared_bodies<'input, 'syntax>(
    input: &'input CompileUnitInput<'syntax>,
    facts: BodyProgramFacts<'input>,
    types: &'input mut TypeStore,
    copyabilities: &'input mut CopyabilityTable,
    closures: &'input mut ClosureTableBuilder,
    body_sources: &BodySourceCatalog<'syntax>,
    body_names: &'input Arena<BodyId, ResolvedBodyNames>,
) -> Result<CheckedBodiesOutput, BodyCheckError> {
    let mut checked_bodies = Vec::new();
    let mut projections = Vec::new();
    let mut opaque_witnesses = Vec::new();
    for (body, _) in facts.graph().declarations().bodies().iter() {
        let source = body_sources
            .get(body)
            .ok_or(BodyCheckInternalError::MissingBodySource(body))?;
        let names = body_names
            .get(body)
            .ok_or(BodyCheckInternalError::MissingBodyNames(body))?;
        if names.body() != body {
            return Err(BodyCheckInternalError::BodyIdentityMismatch(body).into());
        }
        let unit = BodyUnitInput {
            source,
            names,
            closure_ids: reserve_body_closures(closures, source),
        };
        let mut checked =
            BodyChecker::new(input, facts, types, copyabilities, closures, unit)?.check()?;
        projections.append(&mut checked.projections);
        if let Some(witness) = checked.opaque_witness {
            opaque_witnesses.push(witness);
        }
        checked_bodies.push((body, checked));
    }
    Ok(CheckedBodiesOutput {
        bodies: checked_bodies,
        projections,
        opaque_witnesses,
    })
}

struct CheckedBodiesOutput {
    bodies: Vec<(BodyId, CheckedBodyOutput)>,
    projections: Vec<NodeProjection>,
    opaque_witnesses: Vec<(nocter_model::OpaqueTypeId, TypeId)>,
}

fn reserve_body_closures(
    closures: &mut ClosureTableBuilder,
    source: BodySource<'_>,
) -> HashMap<NodeId, nocter_model::ClosureId> {
    let mut reserved = HashMap::new();
    let mut pending = vec![source.block()];
    while let Some(node) = pending.pop() {
        if source
            .syntax()
            .node(node)
            .is_some_and(|syntax| syntax.kind() == NodeKind::ClosureExpression)
        {
            reserved.insert(node, closures.reserve(source.body()));
        }
        pending.extend(
            source
                .syntax()
                .children(node)
                .iter()
                .rev()
                .filter_map(|element| match element {
                    SyntaxElement::Node(child) => Some(*child),
                    SyntaxElement::Token(_) | SyntaxElement::Missing(_) => None,
                }),
        );
    }
    reserved
}

fn analyze_checked_body_relations(
    graph: &DeclarationGraph,
    types: &TypeStore,
    drops: &DropTable,
    conformances: &crate::ConformanceTable,
    closures: &crate::ClosureTable,
    body_sources: &crate::BodySourceCatalog<'_>,
    checked_bodies: &[(BodyId, CheckedBodyOutput)],
) -> Result<(crate::ProvenanceTable, crate::LoanTable), BodyCheckError> {
    let provenance_inputs = checked_bodies
        .iter()
        .map(|(body, checked)| {
            let source = body_sources
                .get(*body)
                .ok_or(BodyCheckInternalError::MissingBodySource(*body))?;
            Ok(ProvenanceBodyInput::new(
                source,
                &checked.body,
                &checked.node_origins,
            ))
        })
        .collect::<Result<Vec<_>, BodyCheckError>>()?;
    let provenance =
        analyze_program_provenance(graph, types, conformances, closures, &provenance_inputs)?;
    let loan_inputs = checked_bodies
        .iter()
        .map(|(body, checked)| {
            let source = body_sources
                .get(*body)
                .ok_or(BodyCheckInternalError::MissingBodySource(*body))?;
            Ok(LoanBodyInput::new(
                source,
                &checked.body,
                &checked.node_origins,
            ))
        })
        .collect::<Result<Vec<_>, BodyCheckError>>()?;
    let loans = analyze_program_loans(graph, types, drops, &provenance, closures, &loan_inputs)?;
    Ok((provenance, loans))
}

struct CheckedBodyOutput {
    body: CheckedBody,
    projections: Vec<NodeProjection>,
    node_origins: HashMap<BodyNodeId, SourceOrigin>,
    opaque_witness: Option<(nocter_model::OpaqueTypeId, TypeId)>,
    copy_proofs: CopyProofs,
}

struct BodyUnitInput<'input, 'syntax> {
    source: BodySource<'syntax>,
    names: &'input ResolvedBodyNames,
    closure_ids: HashMap<NodeId, nocter_model::ClosureId>,
}

struct BodyChecker<'input, 'syntax> {
    input: &'input CompileUnitInput<'syntax>,
    graph: &'input DeclarationGraph,
    types: &'input mut TypeStore,
    copyabilities: &'input mut CopyabilityTable,
    closures: &'input mut ClosureTableBuilder,
    drops: &'input DropTable,
    conformances: &'input crate::ConformanceTable,
    construction_surfaces: &'input crate::ConstructionSurfaceTable,
    instance_operations: &'input crate::InstanceOperationTable,
    standard_semantics: &'input crate::StandardSemanticTable,
    source_index: &'input SourceIndex,
    source: BodySource<'syntax>,
    names: &'input ResolvedBodyNames,
    builder: CheckedBodyBuilder,
    uses: HashMap<SyntaxOrigin, NameTarget>,
    consumed_uses: HashSet<SyntaxOrigin>,
    local_declarations: HashMap<SyntaxOrigin, LocalBindingId>,
    capture_declarations: HashMap<SyntaxOrigin, CaptureId>,
    result_type: TypeId,
    projections: Vec<NodeProjection>,
    node_origins: HashMap<BodyNodeId, SourceOrigin>,
    loops: Vec<LoopConstruction>,
    flow_reachable: bool,
    assumptions: Vec<crate::CheckedRequirement>,
    intrinsic_facts: Vec<crate::CheckedPredicate>,
    copy_proofs: CopyProofs,
    closure_result_inference: Option<closure_results::ClosureResultInference>,
    closure_ids: HashMap<NodeId, nocter_model::ClosureId>,
    closure_type_arguments: Box<[TypeId]>,
    opaque_result: Option<OpaqueResultState>,
}

impl<'input, 'syntax> BodyChecker<'input, 'syntax> {
    fn instance_selector(&mut self) -> InstanceOperationSelector<'_> {
        InstanceOperationSelector::new(
            InstanceSelectionContext::new(
                self.graph,
                self.conformances,
                self.instance_operations,
                &self.assumptions,
                &self.intrinsic_facts,
                self.source.module(),
            ),
            self.types,
            self.copyabilities,
        )
    }

    fn new(
        input: &'input CompileUnitInput<'syntax>,
        facts: BodyProgramFacts<'input>,
        types: &'input mut TypeStore,
        copyabilities: &'input mut CopyabilityTable,
        closures: &'input mut ClosureTableBuilder,
        unit: BodyUnitInput<'input, 'syntax>,
    ) -> Result<Self, BodyCheckError> {
        let BodyUnitInput {
            source,
            names,
            closure_ids,
        } = unit;
        let graph = facts.graph();
        let drops = facts.drops();
        let conformances = facts.conformances();
        let construction_surfaces = facts.construction_surfaces();
        let instance_operations = facts.instance_operations();
        let standard_semantics = facts.standard_semantics();
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
        let mut capture_declarations = HashMap::new();
        for (capture, _) in names.captures().iter() {
            let entity = SemanticEntity::Capture(source.body(), capture);
            let origin = source_index
                .bindings_for(entity)
                .iter()
                .find(|binding| binding.role() == SourceRole::Declaration)
                .map(|binding| binding.origin().syntax())
                .ok_or(BodyCheckInternalError::MissingSource(entity))?;
            if capture_declarations.insert(origin, capture).is_some() {
                return Err(BodyCheckInternalError::DuplicateCaptureDeclaration(origin).into());
            }
        }
        let result_type = body_result_type(graph, types, source)?;
        let closure_type_arguments = body_generic_domain(graph, source)?
            .iter()
            .map(|parameter| {
                types
                    .intern(TypeKind::GenericParameter(*parameter))
                    .map_err(|_| BodyCheckInternalError::UnknownType(result_type))
            })
            .collect::<Result<Vec<_>, _>>()?
            .into_boxed_slice();
        let assumptions = body_assumptions(graph, types, conformances, instance_operations, source)
            .map_err(BodyCheckInternalError::BodyAssumptions)?;
        let opaque_result = OpaqueResultState::for_body(graph, types, source, result_type)?;
        Ok(Self {
            input,
            graph,
            types,
            copyabilities,
            closures,
            drops,
            conformances,
            construction_surfaces,
            instance_operations,
            standard_semantics,
            source_index,
            source,
            names,
            builder: CheckedBodyBuilder::new(names),
            uses,
            consumed_uses: HashSet::new(),
            local_declarations,
            capture_declarations,
            result_type,
            projections: Vec::new(),
            node_origins: HashMap::new(),
            loops: Vec::new(),
            flow_reachable: true,
            assumptions: assumptions.declared().to_vec(),
            intrinsic_facts: assumptions.intrinsic().to_vec(),
            copy_proofs: assumptions.copy_proofs().clone(),
            closure_result_inference: None,
            closure_ids,
            closure_type_arguments,
            opaque_result,
        })
    }

    fn check(mut self) -> Result<CheckedBodyOutput, BodyCheckError> {
        let root = self.check_block(self.source.block(), BlockExpectation::Callable)?;
        if self.consumed_uses.len() != self.names.uses().len() {
            return Err(BodyCheckInternalError::UnconsumedNameUses(self.source.body()).into());
        }
        let opaque_witness = self.finish_opaque_witness(self.source.block())?;
        let body = self.builder.finish(root)?;
        Ok(CheckedBodyOutput {
            body,
            projections: self.projections,
            node_origins: self.node_origins,
            opaque_witness,
            copy_proofs: self.copy_proofs,
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
            NodeKind::RegionStatement => {
                let node = self.check_region(executable)?;
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
        let target = self.required_child(statement, NodeKind::BindingTarget)?;
        let token = direct_identifier(self.tree(), target)
            .ok_or(BodyCheckInternalError::InvalidSyntax(target))?;
        let annotation = direct_child(self.tree(), statement, NodeKind::TypeAnnotation);
        let discard = self.token_text(token)? == "_";
        let mutable = self.tree().children(statement).iter().any(|element| {
            matches!(
                element,
                SyntaxElement::Token(token)
                    if token.kind() == TokenKind::Keyword(Keyword::Var)
            )
        });
        if discard && (mutable || annotation.is_some()) {
            return Err(self.rule(BodyRule::InvalidDiscardBinding, target)?);
        }
        let expected = annotation
            .map(|annotation| {
                let ty = self.required_child(annotation, NodeKind::Type)?;
                self.resolve_data_type_use(ty)
            })
            .transpose()?;
        let initializer = self.required_child(statement, NodeKind::Expression)?;
        let value = self.check_expression(initializer, expected)?;
        let ty = self.node_type(value)?;
        if discard {
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
        if self.closure_result_inference.is_some() {
            let (payload, evidence) = if let Some(expression) =
                direct_child(self.tree(), statement, NodeKind::Expression)
            {
                if closure_results::is_absent_expression(self, expression) {
                    (None, ExpectedEvidence::Absent)
                } else {
                    let value = self.check_expression(expression, None)?;
                    let ty = self.node_type(value)?;
                    let evidence = if ty == self.types.builtin(BuiltinType::Error) {
                        ExpectedEvidence::Failure
                    } else {
                        ExpectedEvidence::Typed(ty)
                    };
                    (Some(value), evidence)
                }
            } else {
                let completion = self.add_node(
                    statement,
                    self.types.builtin(BuiltinType::Void),
                    CheckedOperation::Complete,
                )?;
                (
                    Some(completion),
                    ExpectedEvidence::Typed(self.types.builtin(BuiltinType::Void)),
                )
            };
            let control = self.add_node(
                statement,
                self.types.builtin(BuiltinType::Never),
                CheckedOperation::Control(CheckedControl::Return(payload)),
            )?;
            self.record_inferred_closure_return(statement, control, payload, evidence)?;
            return Ok(control);
        }
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
        if self.is_region_root(root)
            || matches!(self.types.get(ty), Some(TypeKind::Borrow { .. }))
            || self.classify_copyability(ty)? == Copyability::Copy
        {
            return Err(self.rule(BodyRule::InvalidExplicitDrop, statement)?);
        }
        let place = self.builder.add_place(
            root,
            Vec::<PlaceProjection>::new(),
            Vec::<TypeId>::new(),
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
                NodeKind::StructLiteral => return self.check_struct_literal(current, expected),
                NodeKind::ArrayLiteral => return self.check_array_literal(current, expected),
                NodeKind::TypedSequenceLiteral => {
                    return self.check_typed_sequence_literal(current, expected);
                }
                NodeKind::TypedStringLiteral => {
                    return self.check_typed_string_literal(current, expected);
                }
                NodeKind::StringExpression => {
                    return self.check_string_expression(current, expected);
                }
                NodeKind::ClosureExpression => return self.check_closure(current, expected),
                NodeKind::IfExpression => return self.check_if(current, expected),
                NodeKind::MatchExpression => return self.check_match(current, expected),
                NodeKind::AdditiveExpression | NodeKind::MultiplicativeExpression => {
                    return self.check_arithmetic(current, expected);
                }
                NodeKind::ShiftExpression => return self.check_shift(current, expected),
                NodeKind::EqualityExpression | NodeKind::OrderingExpression => {
                    return self.check_comparison(current, expected);
                }
                NodeKind::LogicalAndExpression | NodeKind::LogicalOrExpression => {
                    return self.check_logical(current, expected);
                }
                NodeKind::PostfixExpression
                    if direct_child(self.tree(), current, NodeKind::CallSuffix).is_some() =>
                {
                    return self.check_call(current, expected);
                }
                NodeKind::PostfixExpression => {
                    if let Some((owner, member)) = calls::construction_member_syntax(self, current)?
                    {
                        return self
                            .check_inferred_construction_member(current, owner, member, expected);
                    }
                    return self.check_postfix_reference(current, expected);
                }
                NodeKind::GenericOwnerMember => {
                    return self.check_explicit_construction_member(current, expected);
                }
                NodeKind::ReferenceExpression => {
                    return self.check_reference(current, expected);
                }
                NodeKind::MoveExpression => self.check_move(current)?,
                NodeKind::ConversionExpression => {
                    return self.check_conversion(current, expected);
                }
                NodeKind::OutcomeExpression => {
                    return self.check_outcome_expression(current, expected);
                }
                NodeKind::RecoveryExpression => {
                    return self.check_recovery_expression(current, expected);
                }
                NodeKind::UnaryExpression => return self.check_unary(current, expected),
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
            return self.check_pattern_if(node, condition, expected);
        }
        let condition_expression = self.required_child(condition, NodeKind::Expression)?;
        let condition = self.check_expression(
            condition_expression,
            Some(self.types.builtin(BuiltinType::Bool)),
        )?;
        let branches = self.check_if_branches(node, expected)?;
        let checked = self.add_node(
            node,
            branches.ty,
            CheckedOperation::Control(CheckedControl::If {
                condition,
                then_branch: branches.then_branch,
                else_branch: branches.else_branch,
            }),
        )?;
        expected.map_or(Ok(checked), |expected| {
            self.apply_expected(node, checked, expected)
        })
    }

    fn check_if_branches(
        &mut self,
        node: NodeId,
        expected: Option<TypeId>,
    ) -> Result<CheckedIfBranches, BodyCheckError> {
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
        Ok(CheckedIfBranches {
            then_branch,
            else_branch,
            ty,
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
                    CheckedOperation::Constant(ConstantValue::Integer(i128::from(value))),
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

    fn check_reference(
        &mut self,
        node: NodeId,
        expected: Option<TypeId>,
    ) -> Result<BodyNodeId, BodyCheckError> {
        let place = self.named_place(node)?;
        if let Some(expected) = expected {
            self.apply_expected_place(node, place.id, place.ty, expected)
        } else {
            self.add_node(node, place.ty, CheckedOperation::Copy(place.id))
        }
    }

    fn check_postfix_reference(
        &mut self,
        node: NodeId,
        expected: Option<TypeId>,
    ) -> Result<BodyNodeId, BodyCheckError> {
        let place = self.postfix_place(node, BorrowCapability::Readonly)?;
        if let Some(expected) = expected {
            self.apply_expected_place(node, place.id, place.ty, expected)
        } else {
            self.add_node(node, place.ty, CheckedOperation::Copy(place.id))
        }
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
            return self.check_outcome_expression(node, None);
        }
        self.check_move_place(node)
    }

    fn check_move_place(&mut self, node: NodeId) -> Result<BodyNodeId, BodyCheckError> {
        let operand = self.required_child(node, NodeKind::NamedPlace)?;
        let place = self.named_place(operand)?;
        if self.is_region_place(place.id)? {
            return Err(self.rule(BodyRule::InvalidMoveSource, node)?);
        }
        match self.classify_copyability(place.ty)? {
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

    fn classify_copyability(&mut self, ty: TypeId) -> Result<Copyability, BodyCheckError> {
        self.copyabilities
            .classify_with_proofs(self.graph, self.types, ty, &self.copy_proofs)
            .map_err(BodyCheckInternalError::Copyability)
            .map_err(Into::into)
    }

    fn add_node(
        &mut self,
        syntax: NodeId,
        ty: TypeId,
        operation: CheckedOperation,
    ) -> Result<BodyNodeId, BodyCheckError> {
        if let CheckedOperation::Copy(place) | CheckedOperation::Move(place) = &operation
            && self.is_region_place(*place)?
        {
            return Err(self.rule(BodyRule::InvalidMoveSource, syntax)?);
        }
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
