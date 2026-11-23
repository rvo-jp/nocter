//! Required non-waiting and non-suspending effects of the checked destructor lifecycle.
use super::{DeclarationContext, Failure, Unit, flow};
use crate::value_access::{
    CallKind, ExpressionKind, InstanceControl, InstantiationFailure, ResourceDataFailure,
    ResourceDataProjection, results,
};
use flow::{
    Location, Operation,
    resources::{Disposal, Holder, Step},
};
use nocter_constant_evaluation::state::WorkBudget;
use nocter_declarations::value_access::{CodeInstanceKey, CodeKind, DataInstanceKey};
use nocter_diagnostics::{DiagnosticCode, DiagnosticNote, SourceDiagnostic};
use nocter_model::{
    AccessCodeId, AccessExpressionId, AccessLocalId, BuiltinType, ValueApplication, ValueCode,
    ValueInputOperation,
};
use nocter_runtime_contract::RuntimeAbiIdentity;
use std::{
    borrow::Cow,
    collections::{BTreeMap, BTreeSet},
    fmt,
    sync::Arc,
};

mod protection;
use protection::Protected;

#[cfg(test)]
mod tests;

const INVALID: Failure = Failure::Integrity(flow::IntegrityError::InconsistentTypedBody);
const WORK: &str = "destructor effect analysis exhausted its work budget";
const CLEANUP: &str = "this cleanup has no verified non-waiting implementation";

pub(super) fn qualify(
    context: &DeclarationContext<'_>,
    unit: &Unit,
    work: &mut WorkBudget,
) -> Result<BTreeMap<CodeInstanceKey, DataInstanceKey>, Failure> {
    if !unit.program.has_destructors() {
        return Ok(BTreeMap::new());
    }
    let mut proof = Proof::new(unit, work);
    for (root, declaration) in unit.program.codes().iter() {
        let CodeKind::Destructor(owner) = declaration.kind() else {
            continue;
        };
        let data = unit.program.nominal_type_identity(owner).ok_or(INVALID)?;
        let Ok(receiver) = unit.program.instantiate_closed_data(data) else {
            // A compound receiver can contain a callable whose representation is supplied by
            // the actual value at retirement. Neither a nominal identity nor a signature
            // represents that witness. Concrete lifecycle admission checks the exact instance.
            continue;
        };
        proof
            .require(root, receiver)
            .map_err(|error| rejection(context, unit, root, error))?;
    }
    Ok(proof.completions)
}

/// The checked code and operation that made an exact receiver's destruction ineligible.
#[derive(Clone)]
pub struct DestructorFailure {
    root: Option<AccessCodeId>,
    failure: CheckFailure,
    evidence: Arc<results::Evidence>,
}

impl fmt::Debug for DestructorFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DestructorFailure")
            .field("root", &self.root)
            .field("failure", &self.failure)
            .finish_non_exhaustive()
    }
}

impl PartialEq for DestructorFailure {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.evidence, &other.evidence)
            && self.root == other.root
            && self.failure == other.failure
    }
}
impl Eq for DestructorFailure {}

impl DestructorFailure {
    /// Bind this rejection against the exact current declaration sources.
    #[must_use]
    pub fn bind(&self, unit: &Unit, context: &DeclarationContext<'_>) -> Failure {
        if !Arc::ptr_eq(&self.evidence, &unit.result_evidence) {
            return INVALID;
        }
        self.root
            .map_or(INVALID, |root| rejection(context, unit, root, self.failure))
    }
}

pub(super) fn qualify_receiver(
    unit: &Unit,
    receiver: DataInstanceKey,
    work: &mut WorkBudget,
) -> Result<BTreeMap<CodeInstanceKey, DataInstanceKey>, DestructorFailure> {
    let Some(root) = unit
        .program
        .data_destructor(receiver.data())
        .filter(|_| !receiver.is_uninhabited())
    else {
        return Err(DestructorFailure {
            root: None,
            failure: CheckFailure::Integrity(flow::IntegrityError::InconsistentTypedBody),
            evidence: Arc::clone(&unit.result_evidence),
        });
    };
    let mut proof = Proof::new(unit, work);
    proof
        .require(root, receiver)
        .map_err(|failure| DestructorFailure {
            root: Some(root),
            failure,
            evidence: Arc::clone(&unit.result_evidence),
        })?;
    Ok(proof.completions)
}

struct Proof<'a, 'work> {
    unit: &'a Unit,
    frames: BTreeSet<Frame>,
    completions: BTreeMap<CodeInstanceKey, DataInstanceKey>,
    retired: BTreeSet<DataInstanceKey>,
    pending: Vec<Frame>,
    work: &'work mut WorkBudget,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct Frame {
    key: CodeInstanceKey,
    protected: BTreeSet<AccessLocalId>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CheckFailure {
    Integrity(flow::IntegrityError),
    Rejected {
        code: ValueCode,
        at: Option<Location>,
        message: &'static str,
    },
}

impl CheckFailure {
    fn at_entry(code: ValueCode, message: &'static str) -> Self {
        Self::Rejected {
            code,
            at: None,
            message,
        }
    }
}

impl<'a, 'work> Proof<'a, 'work> {
    fn new(unit: &'a Unit, work: &'work mut WorkBudget) -> Self {
        Self {
            unit,
            frames: BTreeSet::new(),
            completions: BTreeMap::new(),
            retired: BTreeSet::new(),
            pending: Vec::new(),
            work,
        }
    }

    fn require(&mut self, code: AccessCodeId, data: DataInstanceKey) -> Result<(), CheckFailure> {
        self.retire(data)
            .map_err(|message| CheckFailure::at_entry(code.into(), message))?;
        while let Some(frame) = self.pending.pop() {
            if self.frames.insert(frame.clone()) {
                self.body(&frame)?;
            }
        }
        Ok(())
    }

    fn body(&mut self, frame: &Frame) -> Result<(), CheckFailure> {
        let code = frame.key.code();
        let body = self.unit.code_body(code).ok_or(CheckFailure::Integrity(
            flow::IntegrityError::InconsistentTypedBody,
        ))?;
        let protected =
            Protected::new(body.typed(), &frame.protected).map_err(CheckFailure::Integrity)?;
        let control = self
            .unit
            .instantiate_control(&frame.key, self.work)
            .map_err(|failure| CheckFailure::at_entry(code, metadata_failure(failure)))?;
        for (point, block) in body.graph().points().iter() {
            if !control.reachable(point) {
                continue;
            }
            for (action, operation) in block.actions.iter().enumerate() {
                let at = Location { point, action };
                self.operation(body, &control, &protected, at, &operation.operation)
                    .map_err(|message| CheckFailure::Rejected {
                        code,
                        at: Some(at),
                        message,
                    })?;
            }
        }
        if let ValueCode::Declaration(id) = code
            && self
                .unit
                .program
                .codes()
                .get(id)
                .is_some_and(|declaration| matches!(declaration.kind(), CodeKind::Destructor(_)))
        {
            let completed = self.completed_receiver(body, &control)?;
            for component in completed
                .cleanup_components(&self.unit.program)
                .map_err(|_| CheckFailure::at_entry(code, CLEANUP))?
            {
                self.retire(component)
                    .map_err(|message| CheckFailure::at_entry(code, message))?;
            }
            if let Some(previous) = self
                .completions
                .insert(frame.key.clone(), completed.clone())
                && previous != completed
            {
                return Err(CheckFailure::Integrity(
                    flow::IntegrityError::InconsistentTypedBody,
                ));
            }
        }
        Ok(())
    }

    fn completed_receiver(
        &mut self,
        body: &flow::Query,
        control: &InstanceControl,
    ) -> Result<DataInstanceKey, CheckFailure> {
        let frame = control.frame();
        let code = frame.code();
        let [receiver] = body.typed().body().inputs().explicit() else {
            return Err(CheckFailure::Integrity(
                flow::IntegrityError::InconsistentTypedBody,
            ));
        };
        let Some(entry) = frame.input_data(0) else {
            return self
                .unit
                .instantiate_input_data(frame, receiver.local, self.work)
                .map_err(|failure| CheckFailure::at_entry(code, metadata_failure(failure)));
        };
        if !body
            .graph()
            .points()
            .iter()
            .any(|(point, _)| matches!(control.terminator(point), Some(flow::Terminator::Return)))
        {
            return Ok(entry.clone());
        }
        let completed = self
            .unit
            .instantiate_completed_input_data(frame, receiver.local, self.work)
            .map_err(|failure| CheckFailure::at_entry(code, metadata_failure(failure)))?;
        if !entry.has_same_representation(&self.unit.program, &completed) {
            return Err(CheckFailure::at_entry(
                code,
                "the completed receiver has a different cleanup representation",
            ));
        }
        Ok(completed)
    }

    fn operation(
        &mut self,
        body: &flow::Query,
        control: &InstanceControl,
        protected: &Protected<'_>,
        at: Location,
        operation: &Operation,
    ) -> Result<(), &'static str> {
        self.work.step().map_err(|_| WORK)?;
        match *operation {
            Operation::DriveFuture { .. } => {
                return Err("destruction cannot drive a potentially suspending computation");
            }
            Operation::Assign { target, .. } => protected.check_replacement(target)?,
            Operation::CommitCall(call) => self.call(body, control, protected, call)?,
            _ => {}
        }
        for transition in body.access().resources().at(at) {
            let code = control.frame().code();
            for step in transition.steps() {
                if let Step::Dispose { source, payload } = step
                    && matches!(payload.disposal(), Disposal::UnstartedFuture(_))
                {
                    self.future_cancellation(code, at, source.holder())?;
                }
            }
            let before = self.ordinary_steps(code, transition.before_steps());
            let after = self.ordinary_steps(code, transition.after_steps());
            if !before
                .iter()
                .chain(after.iter())
                .any(|step| step.retired_payload().is_some())
            {
                continue;
            }
            let mut projection = ResourceDataProjection::new(
                &self.unit.program,
                &self.unit.result_evidence,
                Some(body),
                control.frame(),
                at,
                Some(control),
                self.work,
            )
            .map_err(resource_failure)?;
            let before = projection.steps(&before).map_err(resource_failure)?;
            let after = projection.after_steps(&after).map_err(resource_failure)?;
            for payload in before
                .iter()
                .chain(&after)
                .filter_map(Step::retired_payload)
            {
                self.retire(payload.instance().clone())?;
            }
        }
        Ok(())
    }

    fn call(
        &mut self,
        body: &flow::Query,
        control: &InstanceControl,
        protected: &Protected<'_>,
        call: AccessExpressionId,
    ) -> Result<(), &'static str> {
        let frame = control.frame();
        let site = ValueApplication {
            owner: frame.code(),
            call,
        };
        let Some(ExpressionKind::Call {
            kind, arguments, ..
        }) = body
            .typed()
            .body()
            .expressions()
            .get(call)
            .map(|expression| &expression.kind)
        else {
            return Err(metadata_failure(InstantiationFailure::MissingCall));
        };
        let consumes_environment = *kind == CallKind::Value
            && arguments
                .first()
                .is_some_and(|argument| argument.operation == ValueInputOperation::Take);
        let protected_positions =
            protected.positions(kind.explicit_arguments(arguments).ok_or(CLEANUP)?)?;
        let program = &self.unit.program;
        let evidence = &self.unit.result_evidence;
        match results::instantiate_controlled_invocation(
            program, evidence, frame, site, control, self.work,
        ) {
            Ok(selected) => self.invoke(selected, consumes_environment, &protected_positions),
            Err(InstantiationFailure::Unresolved) => {
                let domain = results::instantiate_controlled_invocation_domain(
                    program, evidence, frame, site, control, self.work,
                )
                .map_err(metadata_failure)?;
                for case in domain.cases() {
                    self.invoke(
                        case.frame().clone(),
                        consumes_environment,
                        &protected_positions,
                    )?;
                }
                Ok(())
            }
            Err(failure) => Err(metadata_failure(failure)),
        }
    }

    fn ordinary_steps<'steps>(
        &self,
        code: ValueCode,
        steps: &'steps [Step],
    ) -> Cow<'steps, [Step]> {
        let local = |step: &Step| self.unit.local_futures.contains_step(code, step);
        if steps.iter().any(local) {
            Cow::Owned(steps.iter().filter(|step| !local(step)).cloned().collect())
        } else {
            Cow::Borrowed(steps)
        }
    }

    fn invoke(
        &mut self,
        selected: CodeInstanceKey,
        consumes_environment: bool,
        protected_positions: &[usize],
    ) -> Result<(), &'static str> {
        if let ValueCode::Declaration(code) = selected.code()
            && let Some(role) = self.unit.primitives().role(code)
        {
            if !protected_positions.is_empty() {
                return Err("a primitive cannot take over the receiver during destruction");
            }
            if role.execution_facts().may_block() {
                return Err("this primitive may wait for external progress during destruction");
            }
            // These certificates cover the actual selected backing and access to stable input
            // or process snapshots. Unknown observations cannot justify a non-waiting cycle.
            if self
                .unit
                .destruction_abi
                .is_none_or(|abi| role.value_drive_backing(abi).is_none())
            {
                return Err("this primitive has no non-waiting certificate on the selected target");
            }
        } else {
            if consumes_environment && matches!(selected.code(), ValueCode::Closure(_)) {
                self.retire_adapter_environment(&selected)?;
            }
            let body = self
                .unit
                .code_body(selected.code())
                .ok_or(CLEANUP)?
                .typed()
                .body();
            let protected = protected_positions
                .iter()
                .map(|&position| {
                    let input = body.inputs().explicit().get(position).ok_or(CLEANUP)?;
                    if input.operation != ValueInputOperation::Edit {
                        return Err(CLEANUP);
                    }
                    Ok(input.local)
                })
                .collect::<Result<_, _>>()?;
            self.pending.push(Frame {
                key: selected,
                protected,
            });
        }
        Ok(())
    }

    fn retire_adapter_environment(&mut self, frame: &CodeInstanceKey) -> Result<(), &'static str> {
        let body = self
            .unit
            .code_body(frame.code())
            .ok_or(CLEANUP)?
            .typed()
            .body();
        let input = body.inputs().environment().ok_or(CLEANUP)?;
        if input.operation == ValueInputOperation::Take {
            // The selected body owns the environment; its own retirement steps are checked.
            return Ok(());
        }
        let data = body.locals().get(input.local).ok_or(CLEANUP)?.data;
        let completed = match self.unit.program.instantiate_closed_data(data) {
            Ok(closed) => closed,
            Err(_) => self
                .unit
                .instantiate_completed_input_data(frame, input.local, self.work)
                .map_err(metadata_failure)?,
        };
        self.retire(completed)
    }

    fn future_cancellation(
        &mut self,
        code: ValueCode,
        at: Location,
        holder: Holder,
    ) -> Result<(), &'static str> {
        let cleanup = self
            .unit
            .local_futures
            .cancellation(code, at, holder)
            .ok_or(CLEANUP)?;
        for input in cleanup.fields() {
            let data = self
                .unit
                .program
                .instantiate_closed_data(input.payload.data())
                .map_err(|_| CLEANUP)?;
            self.retire(data)?;
        }
        Ok(())
    }

    fn retire(&mut self, data: DataInstanceKey) -> Result<(), &'static str> {
        let program = &self.unit.program;
        let mut pending = vec![data];
        while let Some(data) = pending.pop() {
            if data.is_uninhabited() || !self.retired.insert(data.clone()) {
                continue;
            }
            self.work.step().map_err(|_| WORK)?;
            if program.data_destructor(data.data()).is_some() {
                let key = program.bind_destructor(&data).map_err(|_| CLEANUP)?;
                let body = self
                    .unit
                    .code_body(key.code())
                    .ok_or(CLEANUP)?
                    .typed()
                    .body();
                let [receiver] = body.inputs().explicit() else {
                    return Err(CLEANUP);
                };
                self.pending.push(Frame {
                    key,
                    protected: BTreeSet::from([receiver.local]),
                });
                continue;
            }
            if program.data_type(BuiltinType::Error) == Some(data.data())
                && !self
                    .unit
                    .destruction_abi
                    .is_some_and(RuntimeAbiIdentity::can_release_error_during_drive)
            {
                return Err(CLEANUP);
            }
            pending.extend(data.cleanup_components(program).map_err(|_| CLEANUP)?);
        }
        Ok(())
    }
}

fn rejection(
    context: &DeclarationContext<'_>,
    unit: &Unit,
    root: AccessCodeId,
    error: CheckFailure,
) -> Failure {
    let (code, at, message) = match error {
        CheckFailure::Rejected { code, at, message } => (code, at, message),
        CheckFailure::Integrity(error) => return Failure::Integrity(error),
    };
    let project = || {
        let verified = unit.result_evidence.verify(context).ok()?;
        let destructor = context
            .source_origin(context.code_source(root)?.origin())
            .ok()?;
        let mut notes = Vec::new();
        let primary = if let Some(at) = at {
            let bound = unit.code_body(code)?.bind_verified(&verified).ok()?;
            let origin = bound.origin(at).ok()?;
            notes.push(DiagnosticNote::new(
                "this destructor requires verified cleanup",
                destructor,
            ));
            context.source_origin(origin).ok()?
        } else {
            destructor
        };
        Some(Failure::Flow {
            callable: root,
            diagnostics: Box::new([SourceDiagnostic::new(
                DiagnosticCode::E0291,
                message,
                primary,
                notes,
                None::<&str>,
            )]),
        })
    };
    project().unwrap_or(INVALID)
}

const fn metadata_failure(failure: InstantiationFailure) -> &'static str {
    match failure {
        InstantiationFailure::WorkLimit => WORK,
        _ => "this operation has no verified invocation context during destruction",
    }
}

const fn resource_failure(failure: ResourceDataFailure) -> &'static str {
    match failure {
        ResourceDataFailure::Instantiation(InstantiationFailure::WorkLimit) => WORK,
        _ => CLEANUP,
    }
}
