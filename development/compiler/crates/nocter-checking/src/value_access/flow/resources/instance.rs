//! Canonical contextual payload selection; source operations and ownership remain separate.
use super::{Disposal, INVALID, Payload, Program, Result, payload};
use nocter_model::TypeId;
/// A logical payload specialized through one complete declaration-owned data context.
/// This grants no source acceptance, initialized owner or stored representation.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct InstantiatedPayload {
    instance: nocter_declarations::value_access::DataInstanceKey,
    disposal: Disposal,
}
impl InstantiatedPayload {
    /// Rebuild this actual callable with one matched environment context. This retains its code
    /// and invocation view, without changing the original receipt or granting storage/ownership.
    /// # Errors
    /// Rejects non-callables, code tokens, incompatible environment schemas and unresolved data.
    pub fn with_environment(
        &self,
        program: &Program,
        environment: &nocter_declarations::value_access::DataInstanceKey,
    ) -> Result<Self> {
        let nocter_declarations::value_access::InstanceSchema::Function(selected) =
            self.instance.schema(program).map_err(|_| INVALID)?
        else {
            return Err(INVALID);
        };
        let selected = program
            .bind_callable_data(selected.witness(), environment)
            .map_err(|_| INVALID)?;
        let instance = program
            .instantiate_function(self.data(), &selected)
            .map_err(|_| INVALID)?;
        payload(program, self.data())?.instantiate(program, &instance)
    }
    #[must_use]
    pub const fn instance(&self) -> &nocter_declarations::value_access::DataInstanceKey {
        &self.instance
    }
    #[must_use]
    pub const fn data(&self) -> TypeId {
        self.instance.data()
    }
    #[must_use]
    pub const fn disposal(&self) -> Disposal {
        self.disposal
    }
    /// Project the selected function value's own environment, preserving its complete context.
    /// # Errors
    /// Rejects unresolved or unsupported positive environment data.
    pub fn environment(&self, program: &Program) -> Result<Option<Self>> {
        self.instance
            .environment(program)
            .map_err(|_| INVALID)?
            .map(|key| payload(program, key.data())?.instantiate(program, &key))
            .transpose()
    }
}
impl Payload {
    /// Preserve complete callable context while specializing this schema's cleanup obligation.
    /// # Errors
    /// Rejects unresolved producers/bindings and unsupported positive payload schemas.
    pub fn instantiate(
        self,
        program: &Program,
        instance: &nocter_declarations::value_access::DataInstanceKey,
    ) -> Result<InstantiatedPayload> {
        if self.data != instance.data() {
            return Err(INVALID);
        }
        let instance = instance.clone();
        let instance = instance
            .function_representation(program)
            .map_err(|_| INVALID)?
            .unwrap_or(instance);
        let contract = payload(program, instance.data())?;
        if contract.disposal == Disposal::FunctionEnvironment {
            return Err(INVALID);
        }
        Ok(InstantiatedPayload {
            instance,
            disposal: contract.disposal,
        })
    }
}
