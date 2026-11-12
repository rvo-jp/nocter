//! Closed data identity, source-name resolution and payload projection. No source syntax lives here.
use super::{AggregateForm, DeclarationId, Program};
use nocter_model::{BuiltinType, FieldPath, NominalTypeId, SymbolTable, TypeId, TypeKind};
use std::collections::BTreeMap;

/// The exact closed shapes admitted by this declaration product, not a general type classifier.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PayloadSchema<'a> {
    Builtin(BuiltinType),
    Aggregate(super::AggregateSchema<'a>),
    Array(ArraySchema),
    Optional(TypeId),
    Fallible(TypeId),
    Function(super::FunctionWitness),
}

/// One schema-validated logical component, independent of physical or packed positions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Component {
    pub field: nocter_model::ValueField,
    pub data: TypeId,
}

/// A construction-time type reference, resolved once before metadata is published.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataReference {
    Builtin(BuiltinType),
    Record(NominalTypeId),
    Fallible(Box<DataReference>),
    Optional {
        payload: Box<DataReference>,
        layers: std::num::NonZeroUsize,
    },
    Tuple(TupleData),
    Array {
        element: Box<DataReference>,
        length: u64,
    },
}

/// A homogeneous closed array descriptor; cardinality never expands its element schema.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArraySchema {
    element: TypeId,
    length: u64,
}
impl ArraySchema {
    #[must_use]
    pub const fn element(self) -> TypeId {
        self.element
    }
    #[must_use]
    pub const fn length(self) -> u64 {
        self.length
    }
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.length == 0
    }
}
/// A structural data reference with the same minimum arity as semantic tuple types.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TupleData(Box<[DataReference]>);
impl TupleData {
    #[must_use]
    pub fn new(
        first: DataReference,
        second: DataReference,
        remaining: impl IntoIterator<Item = DataReference>,
    ) -> Self {
        let mut fields = vec![first, second];
        fields.extend(remaining);
        Self(fields.into())
    }
    #[must_use]
    pub fn as_slice(&self) -> &[DataReference] {
        &self.0
    }
}
impl DataReference {
    pub(super) fn has_repeated_failure(&self) -> bool {
        let mut pending = vec![(self, false)];
        while let Some((data, failure)) = pending.pop() {
            match data {
                Self::Fallible(payload) => {
                    if failure {
                        return true;
                    }
                    pending.push((payload, true));
                }
                Self::Optional { payload, .. } => pending.push((payload, failure)),
                Self::Tuple(fields) => {
                    pending.extend(fields.as_slice().iter().map(|data| (data, false)));
                }
                Self::Array { element, .. } => pending.push((element, false)),
                _ => {}
            }
        }
        false
    }
    pub(super) fn visit_leaves(&self, visit: impl FnMut(&Self)) {
        self.visit(false, visit);
    }
    pub(super) fn visit_payload_leaves(&self, visit: impl FnMut(&Self)) {
        self.visit(true, visit);
    }
    fn visit(&self, payload_only: bool, mut visit: impl FnMut(&Self)) {
        let mut pending = vec![self];
        while let Some(data) = pending.pop() {
            match data {
                Self::Tuple(fields) => pending.extend(fields.as_slice().iter().rev()),
                Self::Optional { payload, .. } => pending.push(payload),
                Self::Fallible(payload) => {
                    visit(&Self::Builtin(BuiltinType::Error));
                    pending.push(payload);
                }
                Self::Array { element, length } => {
                    if !payload_only || *length != 0 {
                        pending.push(element);
                    }
                }
                _ => visit(data),
            }
        }
    }
}
impl From<BuiltinType> for DataReference {
    fn from(data: BuiltinType) -> Self {
        Self::Builtin(data)
    }
}

impl Program {
    #[must_use]
    pub fn data_type(&self, data: BuiltinType) -> Option<TypeId> {
        supported(data).then(|| self.types.store().builtin(data))
    }
    #[must_use]
    pub fn data_kind(&self, data: TypeId) -> Option<BuiltinType> {
        match self.types.store().get(data)? {
            TypeKind::Builtin(builtin) if supported(*builtin) => Some(*builtin),
            _ => None,
        }
    }
    /// The named record identity, including before its payload schema is ready.
    /// This grants no field, containment, Copy or payload-readiness evidence.
    #[must_use]
    pub fn record_identity(&self, data: TypeId) -> Option<NominalTypeId> {
        match self.types.store().get(data)? {
            TypeKind::Nominal {
                definition,
                arguments,
            } if arguments.as_slice().is_empty()
                && self
                    .namespace
                    .values()
                    .any(|&id| id == DeclarationId::Record(*definition)) =>
            {
                Some(*definition)
            }
            _ => None,
        }
    }
    /// Source-name lookup for ready payload data; semantic equality uses resolved `TypeId` only.
    #[must_use]
    pub fn type_named(&self, spelling: &str) -> Option<TypeId> {
        self.type_named_in(super::ScopeId::ROOT, spelling)
    }
    #[must_use]
    pub fn type_named_in(&self, scope: super::ScopeId, spelling: &str) -> Option<TypeId> {
        let data = self.type_identity_named_in(scope, spelling)?;
        self.data_schema(data).map(|_| data)
    }
    /// Resolve a reserved data identity without granting payload, field or Copy readiness.
    #[must_use]
    pub fn type_identity_named(&self, spelling: &str) -> Option<TypeId> {
        self.type_identity_named_in(super::ScopeId::ROOT, spelling)
    }
    #[must_use]
    pub fn type_identity_named_in(&self, scope: super::ScopeId, spelling: &str) -> Option<TypeId> {
        match data_reference(&self.symbols, &self.namespace, scope, spelling)? {
            DataReference::Builtin(data) => self.data_type(data),
            DataReference::Record(id) => self
                .types
                .store()
                .identity(&super::records::nominal_key(id)),
            DataReference::Tuple(_)
            | DataReference::Array { .. }
            | DataReference::Optional { .. }
            | DataReference::Fallible(_) => None,
        }
    }
    /// Resolve a closed header name without requiring payload or function readiness.
    /// # Errors
    /// Rejects missing scopes, symbols or type names through the same construction namespace.
    pub fn resolve_data_in(
        &self,
        scope: super::ScopeId,
        name: nocter_model::Symbol,
    ) -> Result<DataReference, super::DeclarationRule> {
        resolve_data_name(&self.symbols, &self.namespace, scope, name)
    }
    /// Structural Copy evidence for the payload, never permission to duplicate an access.
    #[must_use]
    pub fn data_copyable(&self, data: TypeId) -> Option<bool> {
        // Structural tuples can share child types exponentially. Visit each shape once, just as
        // nominal Copy publication does, instead of expanding the represented payload tree.
        let mut copyable = BTreeMap::new();
        let mut pending = vec![(data, false)];
        while let Some((current, expanded)) = pending.pop() {
            if copyable.contains_key(&current) {
                continue;
            }
            let children = match self.data_schema(current)? {
                PayloadSchema::Builtin(builtin) => {
                    copyable.insert(current, builtin_copyable(builtin)?);
                    continue;
                }
                PayloadSchema::Optional(payload) => vec![payload],
                PayloadSchema::Fallible(_) => {
                    copyable.insert(current, false);
                    continue;
                }
                PayloadSchema::Array(array) => {
                    if array.is_empty() {
                        copyable.insert(current, true);
                        continue;
                    }
                    vec![array.element()]
                }
                PayloadSchema::Aggregate(schema) => match schema.form() {
                    AggregateForm::Record(record) => {
                        copyable.insert(current, record.copyable());
                        continue;
                    }
                    AggregateForm::Tuple(fields) => fields.to_vec(),
                },
                PayloadSchema::Function(witness) => match witness.origin {
                    nocter_model::ValueFunctionOrigin::Declaration(_) => {
                        copyable.insert(current, true);
                        continue;
                    }
                    nocter_model::ValueFunctionOrigin::Closure(id) => {
                        vec![self.closure(id)?.environment()]
                    }
                    _ => {
                        copyable.insert(current, false);
                        continue;
                    }
                },
            };
            if expanded {
                copyable.insert(
                    current,
                    children
                        .iter()
                        .all(|child| copyable.get(child) == Some(&true)),
                );
            } else {
                pending.push((current, true));
                pending.extend(children.into_iter().rev().map(|child| (child, false)));
            }
        }
        copyable.get(&data).copied()
    }

    /// The exact closed payload shape, without borrow types or generic eligibility fallbacks.
    #[must_use]
    pub fn data_schema(&self, data: TypeId) -> Option<PayloadSchema<'_>> {
        match self.types.store().get(data)? {
            TypeKind::Builtin(builtin) if supported(*builtin) => {
                Some(PayloadSchema::Builtin(*builtin))
            }
            TypeKind::ValueFunction { origin, invocation } => {
                let witness = super::FunctionWitness {
                    origin: *origin,
                    invocation: *invocation,
                };
                // Schema/property queries validate identity, not an allocated signature copy.
                self.function_data(witness)
                    .map(|_| PayloadSchema::Function(witness))
            }
            TypeKind::Nominal {
                definition,
                arguments,
            } if arguments.as_slice().is_empty() => {
                self.records.records.get(*definition).map(|record| {
                    PayloadSchema::Aggregate(super::AggregateSchema::record(record, self.fields()))
                })
            }
            TypeKind::FixedArray { element, length } => Some(PayloadSchema::Array(ArraySchema {
                element: *element,
                length: length.closed_value()?,
            })),
            TypeKind::Optional(payload) => Some(PayloadSchema::Optional(*payload)),
            TypeKind::Fallible(payload) if self.has_failure_layer(*payload) == Some(false) => {
                Some(PayloadSchema::Fallible(*payload))
            }
            TypeKind::Tuple(fields) => Some(PayloadSchema::Aggregate(
                super::AggregateSchema::tuple(fields.as_slice()),
            )),
            _ => None,
        }
    }
    pub(super) fn has_failure_layer(&self, mut data: TypeId) -> Option<bool> {
        loop {
            match self.types.store().get(data)? {
                TypeKind::Optional(payload) => data = *payload,
                TypeKind::Fallible(_) => return Some(true),
                _ => return Some(false),
            }
        }
    }
    /// Validate one concrete logical component against its containing data shape.
    #[must_use]
    pub fn component(
        &self,
        data: TypeId,
        field: nocter_model::ValueField,
    ) -> Option<super::Component> {
        Some(super::Component {
            field,
            data: self.project_component(data, &field, &|index, length| *index < length)?,
        })
    }
    fn project_component<I>(
        &self,
        data: TypeId,
        field: &nocter_model::ValueProjection<I>,
        in_bounds: &impl Fn(&I, u64) -> bool,
    ) -> Option<TypeId> {
        use nocter_model::ValueProjection;
        match (self.data_schema(data)?, field) {
            (PayloadSchema::Aggregate(schema), ValueProjection::Record(id)) => schema
                .component(nocter_model::ValueField::Record(*id))
                .map(|c| c.data),
            (PayloadSchema::Aggregate(schema), ValueProjection::Tuple(position)) => schema
                .component(nocter_model::ValueField::Tuple(*position))
                .map(|c| c.data),
            (
                PayloadSchema::Optional(payload) | PayloadSchema::Fallible(payload),
                ValueProjection::Variant(true),
            ) => Some(payload),
            (PayloadSchema::Fallible(_), ValueProjection::Variant(false)) => {
                self.data_type(BuiltinType::Error)
            }
            (PayloadSchema::Array(array), ValueProjection::Array(index)) => {
                (!array.is_empty() && in_bounds(index, array.length())).then_some(array.element())
            }
            _ => None,
        }
    }
    /// Packed record/tuple ordinal of an established selection, without projection rights.
    /// Homogeneous array positions are not aggregate ordinals and have no encoding here.
    #[must_use]
    pub fn component_position(&self, field: nocter_model::ValueField) -> Option<usize> {
        match field {
            nocter_model::ValueField::Record(id) => Some(self.fields().get(id)?.position()),
            nocter_model::ValueField::Tuple(position) => Some(position),
            nocter_model::ValueField::Variant(primary) => Some(usize::from(!primary)),
            nocter_model::ValueField::Array(_) => None,
        }
    }
    /// Validates an already selected logical path against its exact containing data schemas.
    /// No name lookup, permission inference or physical offset participates in this projection.
    #[must_use]
    pub fn project_data(&self, data: TypeId, path: &FieldPath) -> Option<TypeId> {
        self.project_data_by(data, path, |index, length| *index < length)
    }
    /// Project declared data through caller-proven array positions without inventing numeric
    /// indices. The caller owns each index's bounds evidence; this grants no access or payload.
    #[must_use]
    pub fn project_data_by<I>(
        &self,
        mut data: TypeId,
        path: &nocter_model::ValuePath<I>,
        in_bounds: impl Fn(&I, u64) -> bool,
    ) -> Option<TypeId> {
        self.data_schema(data)?;
        for field in path.fields() {
            data = self.project_component(data, field, &in_bounds)?;
        }
        Some(data)
    }
}

pub(super) fn supported(data: BuiltinType) -> bool {
    builtin_copyable(data).is_some()
}

pub(super) fn builtin_copyable(data: BuiltinType) -> Option<bool> {
    match data {
        BuiltinType::Bool | BuiltinType::Void => Some(true),
        _ if nocter_model::BuiltinNumericType::for_builtin(data)
            .and_then(nocter_model::BuiltinNumericType::integer_bounds)
            .is_some() =>
        {
            Some(true)
        }
        BuiltinType::Error | BuiltinType::Str => Some(false),
        _ => None,
    }
}

pub(super) fn resolve_data_name(
    symbols: &SymbolTable,
    namespace: &super::namespace::Names,
    scope: super::ScopeId,
    name: nocter_model::Symbol,
) -> Result<DataReference, super::DeclarationRule> {
    let spelling = symbols
        .spelling(name)
        .ok_or(super::DeclarationRule::UnknownSymbol)?;
    data_reference(symbols, namespace, scope, spelling)
        .ok_or(super::DeclarationRule::UnknownDataName { name })
}

pub(super) fn data_reference(
    symbols: &SymbolTable,
    namespace: &super::namespace::Names,
    scope: super::ScopeId,
    spelling: &str,
) -> Option<DataReference> {
    if !namespace.contains_scope(scope) {
        return None;
    }
    if let Some(data) = BuiltinType::from_spelling(spelling) {
        return Some(DataReference::Builtin(data));
    }
    match namespace.get(scope, super::Namespace::Type, symbols.get(spelling)?)? {
        DeclarationId::Record(id) => Some(DataReference::Record(id)),
        DeclarationId::Code(_) => None,
    }
}
pub(super) fn resolved_data(
    types: &mut nocter_model::TypeTransaction,
    data: &DataReference,
) -> TypeId {
    let mut pending = vec![(data, false)];
    let mut resolved = Vec::new();
    while let Some((data, expanded)) = pending.pop() {
        let data = match data {
            DataReference::Builtin(data) => types.builtin(*data),
            DataReference::Record(id) => types
                .intern(super::records::nominal_key(*id))
                .expect("validated nominal identity"),
            DataReference::Fallible(payload) => {
                if !expanded {
                    pending.push((data, true));
                    pending.push((payload, false));
                    continue;
                }
                types
                    .intern(TypeKind::Fallible(
                        resolved.pop().expect("resolved fallible payload"),
                    ))
                    .expect("validated fallible reference")
            }
            DataReference::Optional { payload, layers } => {
                if !expanded {
                    pending.push((data, true));
                    pending.push((payload, false));
                    continue;
                }
                let mut payload = resolved.pop().expect("resolved optional payload");
                for _ in 0..layers.get() {
                    payload = types
                        .intern(TypeKind::Optional(payload))
                        .expect("validated optional reference");
                }
                payload
            }
            DataReference::Array { element, length } => {
                if !expanded {
                    pending.push((data, true));
                    pending.push((element, false));
                    continue;
                }
                let element = resolved.pop().expect("resolved array element");
                types
                    .intern(TypeKind::FixedArray {
                        element,
                        length: (*length).into(),
                    })
                    .expect("validated closed array reference")
            }
            DataReference::Tuple(fields) => {
                if !expanded {
                    pending.push((data, true));
                    pending.extend(fields.as_slice().iter().rev().map(|field| (field, false)));
                    continue;
                }
                let fields = resolved.split_off(resolved.len() - fields.as_slice().len());
                types
                    .intern(TypeKind::Tuple(nocter_model::TupleElements::new(
                        fields[0],
                        fields[1],
                        fields[2..].iter().copied(),
                    )))
                    .expect("validated closed tuple reference")
            }
        };
        resolved.push(data);
    }
    resolved.pop().expect("one complete data reference")
}

#[cfg(test)]
mod tests;
