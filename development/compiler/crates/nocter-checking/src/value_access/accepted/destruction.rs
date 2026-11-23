//! Required non-waiting and non-suspending effects of the checked destructor lifecycle.
use super::{Checked, DeclarationContext, Failure, flow};
use crate::value_access::{
    CallKind, ExpressionKind, InstanceControl, InstantiationFailure, ResourceDataFailure,
};
use flow::{
    Location, Operation,
    resources::{Disposal, Holder, Step},
};
use nocter_constant_evaluation::state::WorkBudget;
use nocter_declarations::value_access::{
    CodeInstanceKey, CodeKind, DataInstanceKey, InstanceSchema,
};
use nocter_diagnostics::{DiagnosticCode, DiagnosticNote, SourceDiagnostic};
use nocter_model::{
    AccessCodeId, AccessExpressionId, BuiltinType, ValueApplication, ValueCode, ValueField,
    ValueInputOperation,
};
use nocter_runtime_contract::RuntimeAbiIdentity;
use std::{borrow::Cow, collections::BTreeSet};

#[cfg(test)]
mod tests;

const INVALID: Failure = Failure::Integrity(flow::IntegrityError::InconsistentTypedBody);
const WORK: &str = "destructor effect analysis exhausted its work budget";
const CLEANUP: &str = "this cleanup has no verified non-waiting implementation";

pub(super) fn qualify(
    context: &DeclarationContext<'_>,
    checked: &Checked,
    work: &mut WorkBudget,
) -> Result<BTreeSet<CodeInstanceKey>, Failure> {
    if !checked.program.has_destructors() {
        return Ok(BTreeSet::new());
    }
    let mut proof = Proof {
        context,
        checked,
        abi: context.target().and_then(RuntimeAbiIdentity::for_target),
        frames: BTreeSet::new(),
        retired: BTreeSet::new(),
        pending: Vec::new(),
        work,
    };
    for (root, declaration) in checked.program.codes().iter() {
        let CodeKind::Destructor(owner) = declaration.kind() else {
            continue;
        };
        let data = checked
            .program
            .nominal_type_identity(owner)
            .ok_or(INVALID)?;
        let receiver = checked.program.instantiate_closed_data(data).map_err(|_| {
            proof.rejection(
                root,
                None,
                "the destructor receiver has no closed cleanup context",
            )
        })?;
        proof
            .retire(receiver)
            .map_err(|message| proof.rejection(root, None, message))?;
        while let Some(frame) = proof.pending.pop() {
            if proof.frames.insert(frame.clone()) {
                proof.body(root, &frame)?;
            }
        }
    }
    Ok(proof.frames.into_iter().filter(|frame| {
        matches!(frame.code(), ValueCode::Declaration(code)
            if checked.program.codes().get(code).is_some_and(|declaration| matches!(declaration.kind(), CodeKind::Destructor(_))))
    }).collect())
}

struct Proof<'a, 'source, 'work> {
    context: &'a DeclarationContext<'source>,
    checked: &'a Checked,
    abi: Option<RuntimeAbiIdentity>,
    frames: BTreeSet<CodeInstanceKey>,
    retired: BTreeSet<DataInstanceKey>,
    pending: Vec<CodeInstanceKey>,
    work: &'work mut WorkBudget,
}

impl Proof<'_, '_, '_> {
    fn body(&mut self, root: AccessCodeId, frame: &CodeInstanceKey) -> Result<(), Failure> {
        let body = self.checked.code(frame.code()).ok_or(INVALID)?;
        let control = self
            .checked
            .instantiate_control(frame, self.work)
            .map_err(|failure| self.rejection(root, None, metadata_failure(failure)))?;
        for (point, block) in body.graph().points().iter() {
            if !control.reachable(point) {
                continue;
            }
            for (action, operation) in block.actions.iter().enumerate() {
                let at = Location { point, action };
                self.operation(body, &control, at, &operation.operation)
                    .map_err(|message| self.rejection(root, Some((body, at)), message))?;
            }
        }
        Ok(())
    }

    fn operation(
        &mut self,
        body: &flow::Query,
        control: &InstanceControl,
        at: Location,
        operation: &Operation,
    ) -> Result<(), &'static str> {
        self.work.step().map_err(|_| WORK)?;
        match *operation {
            Operation::DriveFuture { .. } => {
                return Err("destruction cannot drive a potentially suspending computation");
            }
            Operation::CommitCall(call) => self.call(body, control, call)?,
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
            let mut projection = self
                .checked
                .project_resource_data(control.frame(), at, control, self.work)
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
        match self
            .checked
            .instantiate_invocation(frame, site, control, self.work)
        {
            Ok(selected) => self.invoke(selected, consumes_environment),
            Err(InstantiationFailure::Unresolved) => {
                let domain = self
                    .checked
                    .instantiate_invocation_domain(frame, site, control, self.work)
                    .map_err(metadata_failure)?;
                for case in domain.cases() {
                    self.invoke(case.frame().clone(), consumes_environment)?;
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
        let local = |step: &Step| self.checked.local_futures.contains_step(code, step);
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
    ) -> Result<(), &'static str> {
        if let ValueCode::Declaration(code) = selected.code()
            && let Some(role) = self.context.primitives().role(code)
        {
            if role.execution_facts().may_block() {
                return Err("this primitive may wait for external progress during destruction");
            }
            // These certificates cover the actual selected backing and access to stable input
            // or process snapshots. Unknown observations cannot justify a non-waiting cycle.
            if self
                .abi
                .is_none_or(|abi| role.value_drive_backing(abi).is_none())
            {
                return Err("this primitive has no non-waiting certificate on the selected target");
            }
        } else {
            if consumes_environment && matches!(selected.code(), ValueCode::Closure(_)) {
                self.retire_adapter_environment(&selected)?;
            }
            self.pending.push(selected);
        }
        Ok(())
    }

    fn retire_adapter_environment(&mut self, frame: &CodeInstanceKey) -> Result<(), &'static str> {
        let body = self
            .checked
            .code(frame.code())
            .ok_or(CLEANUP)?
            .typed()
            .body();
        let input = body.inputs().environment().ok_or(CLEANUP)?;
        if input.operation == ValueInputOperation::Take {
            // The selected body owns the environment; its own retirement steps are checked.
            return Ok(());
        }
        let data = body.locals().get(input.local).ok_or(CLEANUP)?.data;
        let completed = match self.checked.program.instantiate_closed_data(data) {
            Ok(closed) => closed,
            Err(_) => self
                .checked
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
            .checked
            .local_futures
            .cancellation(code, at, holder)
            .ok_or(CLEANUP)?;
        let (direct, selected) = match cleanup {
            super::FutureCaptureCases::Direct(fields) => (fields.as_ref(), &[][..]),
            super::FutureCaptureCases::Selected(cases) => (&[][..], cases.as_ref()),
        };
        for input in direct
            .iter()
            .chain(selected.iter().flat_map(|(_, fields)| fields.iter()))
        {
            let data = self
                .checked
                .program
                .instantiate_closed_data(input.payload.data())
                .map_err(|_| CLEANUP)?;
            self.retire(data)?;
        }
        Ok(())
    }

    fn retire(&mut self, data: DataInstanceKey) -> Result<(), &'static str> {
        let program = &self.checked.program;
        let mut pending = vec![data];
        while let Some(data) = pending.pop() {
            if data.is_uninhabited() || !self.retired.insert(data.clone()) {
                continue;
            }
            self.work.step().map_err(|_| WORK)?;
            if let Some(destructor) = program.data_destructor(data.data()) {
                if !data.is_context_free() {
                    return Err("context-dependent destructor cleanup is unresolved");
                }
                self.pending.push(
                    program
                        .bind_code(destructor.into(), [])
                        .map_err(|_| CLEANUP)?,
                );
            }
            let fields = match data.schema(program).map_err(|_| CLEANUP)? {
                InstanceSchema::Builtin(BuiltinType::Error) => {
                    if !self
                        .abi
                        .is_some_and(RuntimeAbiIdentity::can_release_error_during_drive)
                    {
                        return Err(CLEANUP);
                    }
                    continue;
                }
                InstanceSchema::Uninhabited | InstanceSchema::Builtin(_) => continue,
                InstanceSchema::Aggregate(aggregate) => {
                    aggregate.components().map(|field| field.field).collect()
                }
                InstanceSchema::Enumeration(schema) => {
                    let mut fields = Vec::new();
                    for &case in schema.variants() {
                        fields.extend(
                            program
                                .variant_components(data.data(), case)
                                .ok_or(CLEANUP)?
                                .map(|field| field.field),
                        );
                    }
                    fields
                }
                InstanceSchema::Array(array) if array.is_empty() => continue,
                InstanceSchema::Array(_) => vec![ValueField::Array(0)],
                InstanceSchema::Optional(_) => vec![ValueField::Variant(true)],
                InstanceSchema::Fallible(_) => {
                    vec![ValueField::Variant(true), ValueField::Variant(false)]
                }
                InstanceSchema::Function(_) => {
                    pending.extend(data.environment(program).map_err(|_| CLEANUP)?);
                    continue;
                }
                InstanceSchema::FunctionAlternatives(_) => {
                    let cases = data
                        .function_representations(program)
                        .map_err(|_| CLEANUP)?
                        .ok_or(CLEANUP)?;
                    pending.extend(cases);
                    continue;
                }
            };
            for field in fields {
                pending.push(data.component(program, field).map_err(|_| CLEANUP)?);
            }
        }
        Ok(())
    }

    fn rejection(
        &self,
        root: AccessCodeId,
        operation: Option<(&flow::Query, Location)>,
        message: &str,
    ) -> Failure {
        let project = || {
            let source = self.context.code_source(root)?;
            let destructor = self.context.source_origin(source.origin()).ok()?;
            let mut notes = Vec::new();
            let primary = if let Some((body, at)) = operation {
                let verified = self.checked.result_evidence.verify(self.context).ok()?;
                let bound = body.bind_verified(&verified).ok()?;
                let origin = bound.origin(at).ok()?;
                notes.push(DiagnosticNote::new(
                    "this destructor requires non-waiting cleanup",
                    destructor,
                ));
                self.context.source_origin(origin).ok()?
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
