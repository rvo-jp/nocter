//! Per-case channel closure, separate from runtime selector admission.
use super::{
    Binding, BodyInput, CallKind, CallTarget, Dispatch, Environment, EnvironmentExit, Failure,
    INVALID, InstanceKey, Invocation, InvocationCase, Payload, ValueInputOperation, WorkBudget,
};

/// Close candidate channels without granting an executable selector.
pub(in super::super) fn bind_domain(
    queries: &nocter_checking::value_access::accepted::InstanceMetadata,
    caller: &nocter_checking::value_access::flow::Query,
    domain: &nocter_checking::value_access::InvocationDomain,
    mut target: impl FnMut(&InstanceKey) -> CallTarget,
    work: &mut WorkBudget,
) -> Result<Invocation, Failure> {
    if domain.caller() != queries.control().frame() || caller.typed().code() != domain.site().owner
    {
        return Err(INVALID);
    }
    let unit = queries.unit();
    let argument = match &caller
        .typed()
        .body()
        .expressions()
        .get(domain.site().call)
        .ok_or(INVALID)?
        .kind
    {
        nocter_checking::value_access::ExpressionKind::Call { arguments, .. } => {
            arguments.first().ok_or(INVALID)?
        }
        _ => return Err(INVALID),
    };
    let mut common: Option<Invocation> = None;
    let mut supplied = None;
    let mut representations = None;
    let mut cases = std::collections::BTreeMap::new();
    for case in domain.cases() {
        if case.source().kind != CallKind::Value {
            return Err(INVALID);
        }
        let mut binding = Binding {
            queries,
            unit,
            frame: domain.caller(),
            caller,
            call: domain.site().call,
            body_frame: case.frame(),
            target: target(case.frame()),
            case: None,
            work,
        };
        // The original preparation remains the physical carrier. Only its selected logical
        // case is substituted into the existing single-call channel verifier below.
        if argument.operation != ValueInputOperation::Read {
            return Err(INVALID);
        }
        let carrier = binding.argument_payload(case.source(), 0, argument)?;
        if supplied
            .as_ref()
            .is_some_and(|previous| previous != &carrier)
        {
            return Err(INVALID);
        }
        binding.work.step().map_err(|_| {
            binding.failure(nocter_checking::value_access::InstantiationFailure::WorkLimit)
        })?;
        if representations.is_none() {
            representations = Some(case_index(&carrier, &mut binding)?);
        }
        let key = representations
            .as_ref()
            .ok_or(INVALID)?
            .1
            .get(case.callee())
            .ok_or(INVALID)?
            .clone();
        let payload = binding.payload(key.data(), &key)?;
        binding.case = Some(&payload);
        let next = binding.bundle(case.source())?;
        let row = retained_case(&key, &next)?;
        if common.as_ref().is_some_and(|previous| {
            previous.kind != next.kind
                || previous.inputs != next.inputs
                || previous.result != next.result
        }) {
            return Err(INVALID);
        }
        if cases.get(&key).is_some_and(|previous| previous != &row) {
            return Err(INVALID);
        }
        cases.insert(key, row);
        common = Some(next);
        supplied = Some(carrier);
    }
    let mut common = common.ok_or(INVALID)?;
    let carrier = supplied.ok_or(INVALID)?;
    let (expected, _) = representations.ok_or(INVALID)?;
    // Every stored case needs a checked target; a subset is not a selector proof.
    if expected.len() != cases.len() || expected.iter().any(|key| !cases.contains_key(key)) {
        return Err(INVALID);
    }
    common.dispatch = Dispatch::Selected {
        carrier,
        cases: expected
            .iter()
            .map(|key| cases.remove(key).ok_or(INVALID))
            .collect::<Result<Box<[_]>, _>>()?,
    };
    Ok(common)
}

fn retained_case(
    key: &nocter_declarations::value_access::DataInstanceKey,
    call: &Invocation,
) -> Result<InvocationCase, Failure> {
    let environment = call.environment();
    if !matches!(
        environment,
        Environment::CodeToken {
            exit: EnvironmentExit::Retained,
            ..
        } | Environment::Capture {
            input: BodyInput {
                operation: ValueInputOperation::Read,
                ..
            },
            exit: EnvironmentExit::Retained,
            ..
        }
    ) {
        return Err(INVALID);
    }
    Ok(InvocationCase {
        callee: key.clone(),
        target: call.target(),
        environment,
    })
}

type CaseIndex = (
    Box<[nocter_declarations::value_access::DataInstanceKey]>,
    std::collections::BTreeMap<
        nocter_declarations::value_access::ConcreteWitness,
        nocter_declarations::value_access::DataInstanceKey,
    >,
);

fn case_index(carrier: &Payload, binding: &mut Binding<'_, '_>) -> Result<CaseIndex, Failure> {
    let keys = carrier
        .instance()
        .function_representations(binding.unit.program())
        .map_err(|_| INVALID)?
        .ok_or(INVALID)?;
    if keys.len() < 2 {
        return Err(INVALID);
    }
    binding
        .work
        .charge(u64::try_from(keys.len()).map_err(|_| INVALID)?)
        .map_err(|_| {
            binding.failure(nocter_checking::value_access::InstantiationFailure::WorkLimit)
        })?;
    let by_witness = keys
        .iter()
        .map(|key| {
            let nocter_declarations::value_access::InstanceSchema::Function(witness) =
                key.schema(binding.unit.program()).map_err(|_| INVALID)?
            else {
                return Err(INVALID);
            };
            Ok((witness, key.clone()))
        })
        .collect::<Result<_, Failure>>()?;
    Ok((keys, by_witness))
}
