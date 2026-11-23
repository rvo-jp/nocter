//! Closed data identity, source-name resolution and payload projection. No source syntax lives here.
use super::{AggregateForm, BehaviorOwner, DeclarationId, Program};
use nocter_model::{
    BuiltinType, FieldPath, NominalTypeId, SymbolTable, TypeAliasId, TypeId, TypeKind,
    ValueProjection, VariantId,
};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;
mod header_values;
mod uninhabited;

/// The exact closed shapes admitted by this declaration product, not a general type classifier.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PayloadSchema<'a> {
    Builtin(BuiltinType),
    Enumeration(&'a super::Enumeration),
    Aggregate(super::AggregateSchema<'a>),
    Array(ArraySchema),
    Optional(TypeId),
    Fallible(TypeId),
    Future(TypeId),
    Function(super::FunctionWitness),
}

/// One schema-validated logical component, independent of physical or packed positions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Component {
    pub field: nocter_model::ValueField,
    pub data: TypeId,
}

/// A construction-time type reference, resolved once before metadata is published.
#[derive(Clone, Debug)]
pub enum DataReference {
    Builtin(BuiltinType),
    Nominal(NominalTypeId),
    /// A callable constraint inside an input descriptor, never an erased function pointer.
    Function(Box<super::FunctionDefinition>),
    Fallible(Arc<DataReference>),
    Future(Arc<DataReference>),
    Optional {
        payload: Arc<DataReference>,
        layers: std::num::NonZeroUsize,
    },
    Tuple(TupleData),
    Array {
        element: Arc<DataReference>,
        length: u64,
    },
}

impl PartialEq for DataReference {
    fn eq(&self, other: &Self) -> bool {
        let mut pending = vec![(self, other)];
        let mut seen = HashSet::new();
        while let Some((left, right)) = pending.pop() {
            if std::ptr::eq(left, right)
                || (left.shares_children()
                    && right.shares_children()
                    && !seen.insert((std::ptr::from_ref(left), std::ptr::from_ref(right))))
            {
                continue;
            }
            match (left, right) {
                (Self::Builtin(a), Self::Builtin(b)) if a == b => {}
                (Self::Nominal(a), Self::Nominal(b)) if a == b => {}
                (Self::Fallible(a), Self::Fallible(b)) | (Self::Future(a), Self::Future(b)) => {
                    pending.push((a, b));
                }
                (
                    Self::Optional {
                        payload: a,
                        layers: left,
                    },
                    Self::Optional {
                        payload: b,
                        layers: right,
                    },
                ) if left == right => pending.push((a, b)),
                (
                    Self::Array {
                        element: a,
                        length: left,
                    },
                    Self::Array {
                        element: b,
                        length: right,
                    },
                ) if left == right => pending.push((a, b)),
                (Self::Tuple(a), Self::Tuple(b)) if a.0.len() == b.0.len() => {
                    pending.extend(a.0.iter().zip(b.0.iter()));
                }
                (Self::Function(a), Self::Function(b))
                    if a.invocation == b.invocation && a.inputs.len() == b.inputs.len() =>
                {
                    for ((left, a), (right, b)) in a.inputs.iter().zip(b.inputs.iter()) {
                        if left != right {
                            return false;
                        }
                        pending.push((a, b));
                    }
                    pending.push((&a.result, &b.result));
                }
                _ => return false,
            }
        }
        true
    }
}
impl Eq for DataReference {}

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
pub struct TupleData(Arc<[DataReference]>);
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
    fn shares_children(&self) -> bool {
        match self {
            Self::Tuple(fields) => Arc::strong_count(&fields.0) > 1,
            Self::Optional { payload, .. }
            | Self::Fallible(payload)
            | Self::Future(payload)
            | Self::Array {
                element: payload, ..
            } => Arc::strong_count(payload) > 1,
            Self::Builtin(_) | Self::Nominal(_) | Self::Function(_) => false,
        }
    }

    /// Combine adjacent optional layers into the canonical raw header shape.
    #[must_use]
    pub fn optional(payload: Self, layers: std::num::NonZeroUsize) -> Option<Self> {
        match payload {
            Self::Optional {
                payload,
                layers: inner,
            } => Some(Self::Optional {
                payload,
                layers: std::num::NonZeroUsize::new(layers.get().checked_add(inner.get())?)?,
            }),
            payload => Some(Self::Optional {
                payload: Arc::new(payload),
                layers,
            }),
        }
    }

    pub(super) fn has_repeated_failure(&self) -> bool {
        let mut pending = vec![(self, false)];
        let mut seen = HashSet::new();
        while let Some((data, failure)) = pending.pop() {
            if data.shares_children() && !seen.insert((std::ptr::from_ref(data), failure)) {
                continue;
            }
            match data {
                Self::Fallible(payload) => {
                    if failure {
                        return true;
                    }
                    pending.push((payload, true));
                }
                Self::Optional { payload, .. } => pending.push((payload, failure)),
                Self::Future(output) => pending.push((output, false)),
                Self::Tuple(fields) => {
                    pending.extend(fields.as_slice().iter().map(|data| (data, false)));
                }
                Self::Array { element, .. } => pending.push((element, false)),
                Self::Function(contract) => {
                    pending.push((&contract.result, false));
                    pending.extend(contract.inputs.iter().map(|(_, data)| (data, false)));
                }
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
        let mut seen = HashSet::new();
        while let Some(data) = pending.pop() {
            if data.shares_children() && !seen.insert(std::ptr::from_ref(data)) {
                continue;
            }
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
                Self::Function(contract) => {
                    visit(data);
                    if !payload_only {
                        pending.push(&contract.result);
                        pending.extend(contract.inputs.iter().rev().map(|(_, data)| data));
                    }
                }
                Self::Future(output) => {
                    visit(data);
                    if !payload_only {
                        pending.push(output);
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
    /// Access the canonical type descriptor without granting payload or storage evidence.
    #[must_use]
    pub fn type_kind(&self, data: TypeId) -> Option<&TypeKind> {
        self.types.store().get(data)
    }
    #[must_use]
    pub fn data_kind(&self, data: TypeId) -> Option<BuiltinType> {
        match self.type_kind(data)? {
            TypeKind::Builtin(builtin) if supported(*builtin) => Some(*builtin),
            _ => None,
        }
    }
    /// Existing nominal `TypeId`, independently of payload readiness; never interns a new type.
    #[must_use]
    pub fn nominal_type_identity(&self, nominal: NominalTypeId) -> Option<TypeId> {
        self.types
            .store()
            .identity(&super::nominals::nominal_key(nominal))
    }
    /// The named nominal identity, including before its schema is ready.
    /// This grants no member, containment, Copy or payload-readiness evidence.
    #[must_use]
    pub fn nominal_identity(&self, data: TypeId) -> Option<NominalTypeId> {
        match self.types.store().get(data)? {
            TypeKind::Nominal {
                definition,
                arguments,
            } if arguments.as_slice().is_empty() && self.nominals().is_reserved(*definition) => {
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
        match data_reference(&self.symbols, &self.namespace, scope, spelling) {
            Some(DataReference::Builtin(data)) => self.data_type(data),
            Some(DataReference::Nominal(id)) => self.nominal_type_identity(id),
            _ => self
                .alias(self.type_alias_named_in(scope, spelling)?)
                .map(super::TypeAlias::data),
        }
    }
    /// Resolve an ordinary alias name before asking whether the alias header is ready.
    #[must_use]
    pub fn type_alias_named_in(
        &self,
        scope: super::ScopeId,
        spelling: &str,
    ) -> Option<TypeAliasId> {
        alias_named_in(
            &self.namespace,
            scope,
            self.symbols.get(spelling)?,
            spelling,
        )
    }
    /// Resolve a named type or the owner denoted by `Self` in the current behavior scope.
    #[must_use]
    pub fn type_owner_named_in(
        &self,
        scope: super::ScopeId,
        spelling: &str,
    ) -> Option<BehaviorOwner> {
        self.behavior_owner(self.type_identity_named_in(scope, spelling)?)
    }
    /// Select the owner of additional behavior through normalized data identity.
    #[must_use]
    pub fn behavior_owner(&self, data: TypeId) -> Option<BehaviorOwner> {
        self.data_kind(data)
            .map(BehaviorOwner::Builtin)
            .or_else(|| self.nominal_identity(data).map(BehaviorOwner::Nominal))
    }
    /// Resolve a closed header name without requiring payload or function readiness.
    /// # Errors
    /// Rejects missing scopes, symbols or type names through the same construction namespace.
    pub fn resolve_data_in(
        &self,
        scope: super::ScopeId,
        name: nocter_model::Symbol,
    ) -> Result<DataReference, super::DeclarationRule> {
        resolve_ready_alias(
            resolve_data_name(&self.symbols, &self.namespace, scope, name),
            |id| self.alias(id),
        )
    }
    #[must_use]
    pub fn alias(&self, alias: nocter_model::TypeAliasId) -> Option<&super::TypeAlias> {
        self.aliases.get(alias)
    }
    /// Select type data without requiring the representation of an ordinary nominal declaration.
    #[must_use]
    pub fn type_declaration_identity(&self, declaration: DeclarationId) -> Option<TypeId> {
        match declaration {
            DeclarationId::Nominal(record) => self.nominal_type_identity(record),
            DeclarationId::Alias(alias) => self.alias(alias).map(super::TypeAlias::data),
            DeclarationId::Code(_) => None,
        }
    }
    /// Resolve a selected ordinary type declaration through its ready source meaning.
    /// # Errors
    /// Reports an unresolved alias so a source header can request it before normalization.
    pub fn data_reference_for(
        &self,
        declaration: DeclarationId,
    ) -> Result<Option<DataReference>, super::DeclarationRule> {
        match declaration {
            DeclarationId::Nominal(record) => Ok(Some(DataReference::Nominal(record))),
            DeclarationId::Alias(alias) => self
                .alias(alias)
                .map(|target| Some(target.reference().clone()))
                .ok_or(super::DeclarationRule::UnreadyAlias { alias }),
            DeclarationId::Code(_) => Ok(None),
        }
    }
    /// Find an existing structural identity without interning, publishing or granting payload proof.
    /// Uses the publication owner's data-reference fold; a missing constructor returns no identity.
    #[must_use]
    pub fn existing_data_reference(&self, data: &DataReference) -> Option<TypeId> {
        resolve_reference(data, |node| match node {
            ReferenceNode::Data(TypeKind::Builtin(data)) if !supported(data) => None,
            ReferenceNode::Data(kind) => self.types.store().identity(&kind),
            ReferenceNode::Function { .. } => None,
        })
    }
    /// Match field data against its own published callable contracts without interning or binding
    /// a value. Equal signatures in another field do not substitute for this field's identities.
    #[must_use]
    pub fn existing_field_data_reference(
        &self,
        field: nocter_model::AccessFieldId,
        data: &DataReference,
    ) -> Option<TypeId> {
        self.existing_callable_data_reference(
            data,
            self.fields().get(field)?.callable_contracts(),
            |ordinal| {
                nocter_model::ValueFunctionOrigin::Contract(nocter_model::ValueCallableContract {
                    owner: nocter_model::ValueContractOwner::Field(field),
                    ordinal,
                })
            },
        )
    }
    fn existing_callable_data_reference(
        &self,
        data: &DataReference,
        contracts: &[super::FunctionConstraint],
        origin: impl Fn(usize) -> nocter_model::ValueFunctionOrigin,
    ) -> Option<TypeId> {
        let mut ordinal = 0;
        let resolved = resolve_reference(data, |node| match node {
            ReferenceNode::Data(TypeKind::Builtin(data)) if !supported(data) => None,
            ReferenceNode::Data(kind) => self.types.store().identity(&kind),
            ReferenceNode::Function { contract, .. } => {
                let selected = origin(ordinal);
                let expected = contracts.get(ordinal);
                ordinal += 1;
                (expected == Some(&contract))
                    .then(|| {
                        self.function_data(super::FunctionWitness {
                            origin: selected,
                            invocation: contract.invocation,
                        })
                    })
                    .flatten()
            }
        })?;
        (ordinal == contracts.len()).then_some(resolved)
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
                PayloadSchema::Enumeration(schema) => {
                    copyable.insert(current, schema.copyable());
                    continue;
                }
                PayloadSchema::Optional(payload) => vec![payload],
                PayloadSchema::Fallible(_) | PayloadSchema::Future(_) => {
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
                self.nominals().get(*definition).map(|schema| match schema {
                    super::NominalSchema::Record(record) => PayloadSchema::Aggregate(
                        super::AggregateSchema::record(record, self.fields()),
                    ),
                    super::NominalSchema::Enumeration(schema) => PayloadSchema::Enumeration(schema),
                })
            }
            TypeKind::FixedArray { element, length } => Some(PayloadSchema::Array(ArraySchema {
                element: *element,
                length: length.closed_value()?,
            })),
            TypeKind::Optional(payload) => Some(PayloadSchema::Optional(*payload)),
            TypeKind::Future(output) => Some(PayloadSchema::Future(*output)),
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
    /// Visit one enum case's fields in declaration order. This does not establish an active tag.
    /// # Panics
    /// Panics on iteration if the nominal publication invariant is broken and a case refers to an
    /// unpublished field.
    #[must_use]
    pub fn variant_components(
        &self,
        data: TypeId,
        variant: VariantId,
    ) -> Option<impl ExactSizeIterator<Item = Component> + '_> {
        let case = self.variants().get(variant)?;
        if self.enumeration(case.owner())?.data() != data {
            return None;
        }
        Some(case.fields().iter().map(move |&field| {
            Component {
                field: ValueProjection::EnumField { variant, field },
                data: self
                    .fields()
                    .get(field)
                    .expect("published enum field")
                    .data(),
            }
        }))
    }

    fn project_component<I>(
        &self,
        data: TypeId,
        field: &nocter_model::ValueProjection<I>,
        in_bounds: &impl Fn(&I, u64) -> bool,
    ) -> Option<TypeId> {
        match (self.data_schema(data)?, field) {
            (PayloadSchema::Aggregate(schema), ValueProjection::Record(id)) => schema
                .component(nocter_model::ValueField::Record(*id))
                .map(|c| c.data),
            (PayloadSchema::Aggregate(schema), ValueProjection::Tuple(position)) => schema
                .component(nocter_model::ValueField::Tuple(*position))
                .map(|c| c.data),
            (PayloadSchema::Enumeration(_), ValueProjection::EnumField { variant, field }) => {
                let field = self.fields().get(*field)?;
                (field.variant() == Some(*variant)
                    && self.nominal_identity(data) == Some(field.owner()))
                .then_some(field.data())
            }
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
    /// Flat ordinal of a record, tuple or outcome slot, without projection rights.
    /// Array positions and nominal enum fields need a separate index or case selector.
    #[must_use]
    pub fn component_position(&self, field: nocter_model::ValueField) -> Option<usize> {
        match field {
            nocter_model::ValueField::Record(id) => {
                let field = self.fields().get(id)?;
                field.variant().is_none().then_some(field.position())
            }
            nocter_model::ValueField::Tuple(position) => Some(position),
            nocter_model::ValueField::Variant(primary) => Some(usize::from(!primary)),
            nocter_model::ValueField::EnumField { .. } | nocter_model::ValueField::Array(_) => None,
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
    data_reference(symbols, namespace, scope, spelling).ok_or_else(|| {
        alias_named_in(namespace, scope, name, spelling)
            .map_or(super::DeclarationRule::UnknownDataName { name }, |alias| {
                super::DeclarationRule::UnreadyAlias { alias }
            })
    })
}

pub(super) fn resolve_ready_alias<'a>(
    result: Result<DataReference, super::DeclarationRule>,
    lookup: impl FnOnce(TypeAliasId) -> Option<&'a super::TypeAlias>,
) -> Result<DataReference, super::DeclarationRule> {
    match result {
        Err(rule @ super::DeclarationRule::UnreadyAlias { alias }) => lookup(alias)
            .map(|target| target.reference().clone())
            .ok_or(rule),
        other => other,
    }
}

fn alias_named_in(
    namespace: &super::namespace::Names,
    scope: super::ScopeId,
    name: nocter_model::Symbol,
    spelling: &str,
) -> Option<TypeAliasId> {
    if spelling == "Self" {
        return None;
    }
    let DeclarationId::Alias(alias) = namespace.get(scope, super::Namespace::Type, name)? else {
        return None;
    };
    Some(alias)
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
    if spelling == "Self" {
        return namespace.behavior_owner(scope).map(|owner| match owner {
            BehaviorOwner::Builtin(data) => DataReference::Builtin(data),
            BehaviorOwner::Nominal(record) => DataReference::Nominal(record),
        });
    }
    match namespace.get(scope, super::Namespace::Type, symbols.get(spelling)?)? {
        DeclarationId::Nominal(id) => Some(DataReference::Nominal(id)),
        DeclarationId::Code(_) | DeclarationId::Alias(_) => None,
    }
}
/// The same structural fold supplies publication and read-only identity reconciliation.
pub(super) enum ReferenceNode {
    Data(TypeKind),
    Function {
        contract: super::FunctionConstraint,
        in_signature: bool,
    },
}
pub(super) fn resolve_reference(
    data: &DataReference,
    construct: impl FnMut(ReferenceNode) -> Option<TypeId>,
) -> Option<TypeId> {
    resolve_reference_scoped(data, false, construct)
}

pub(super) fn resolve_function_definition(
    definition: &super::FunctionDefinition,
    mut construct: impl FnMut(ReferenceNode) -> Option<TypeId>,
) -> Option<super::FunctionConstraint> {
    let inputs = definition
        .inputs
        .iter()
        .map(|(operation, data)| {
            Some(super::FunctionInput {
                operation: *operation,
                data: resolve_reference_scoped(data, true, &mut construct)?,
            })
        })
        .collect::<Option<_>>()?;
    Some(super::FunctionConstraint {
        invocation: definition.invocation,
        inputs,
        result: resolve_reference_scoped(&definition.result, true, construct)?,
    })
}

#[expect(
    clippy::too_many_lines,
    reason = "one iterative traversal resolves every closed type constructor"
)]
fn resolve_reference_scoped(
    data: &DataReference,
    in_signature: bool,
    mut construct: impl FnMut(ReferenceNode) -> Option<TypeId>,
) -> Option<TypeId> {
    let mut pending = vec![(data, None, in_signature)];
    let mut resolved = Vec::new();
    let mut shared = HashMap::new();
    let mut callable_occurrences = 0_usize;
    while let Some((data, entered_with, in_signature)) = pending.pop() {
        let key = data.shares_children().then(|| std::ptr::from_ref(data));
        if let Some(id) = key.and_then(|key| shared.get(&key).copied()) {
            resolved.push(id);
            continue;
        }
        let data = match data {
            DataReference::Builtin(data) => {
                construct(ReferenceNode::Data(TypeKind::Builtin(*data)))?
            }
            DataReference::Nominal(id) => {
                construct(ReferenceNode::Data(super::nominals::nominal_key(*id)))?
            }
            DataReference::Function(contract) => {
                if entered_with.is_none() {
                    pending.push((data, Some(callable_occurrences), in_signature));
                    pending.push((&contract.result, None, true));
                    pending.extend(
                        contract
                            .inputs
                            .iter()
                            .rev()
                            .map(|(_, data)| (data, None, true)),
                    );
                    continue;
                }
                let result = resolved.pop().expect("resolved callable result");
                let inputs = resolved.split_off(resolved.len() - contract.inputs.len());
                callable_occurrences = callable_occurrences.checked_add(1)?;
                construct(ReferenceNode::Function {
                    in_signature,
                    contract: super::FunctionConstraint {
                        invocation: contract.invocation,
                        inputs: inputs
                            .into_iter()
                            .zip(contract.inputs.iter())
                            .map(|(data, (operation, _))| super::FunctionInput {
                                data,
                                operation: *operation,
                            })
                            .collect(),
                        result,
                    },
                })?
            }
            DataReference::Fallible(payload) => {
                if entered_with.is_none() {
                    pending.push((data, Some(callable_occurrences), in_signature));
                    pending.push((payload, None, in_signature));
                    continue;
                }
                construct(ReferenceNode::Data(TypeKind::Fallible(
                    resolved.pop().expect("resolved fallible payload"),
                )))?
            }
            DataReference::Future(output) => {
                if entered_with.is_none() {
                    pending.push((data, Some(callable_occurrences), in_signature));
                    pending.push((output, None, in_signature));
                    continue;
                }
                construct(ReferenceNode::Data(TypeKind::Future(
                    resolved.pop().expect("resolved future output"),
                )))?
            }
            DataReference::Optional { payload, layers } => {
                if entered_with.is_none() {
                    pending.push((data, Some(callable_occurrences), in_signature));
                    pending.push((payload, None, in_signature));
                    continue;
                }
                let mut payload = resolved.pop().expect("resolved optional payload");
                for _ in 0..layers.get() {
                    payload = construct(ReferenceNode::Data(TypeKind::Optional(payload)))?;
                }
                payload
            }
            DataReference::Array { element, length } => {
                if entered_with.is_none() {
                    pending.push((data, Some(callable_occurrences), in_signature));
                    pending.push((element, None, in_signature));
                    continue;
                }
                let element = resolved.pop().expect("resolved array element");
                construct(ReferenceNode::Data(TypeKind::FixedArray {
                    element,
                    length: (*length).into(),
                }))?
            }
            DataReference::Tuple(fields) => {
                if entered_with.is_none() {
                    pending.push((data, Some(callable_occurrences), in_signature));
                    pending.extend(
                        fields
                            .as_slice()
                            .iter()
                            .rev()
                            .map(|field| (field, None, in_signature)),
                    );
                    continue;
                }
                let fields = resolved.split_off(resolved.len() - fields.as_slice().len());
                construct(ReferenceNode::Data(TypeKind::Tuple(
                    nocter_model::TupleElements::new(
                        fields[0],
                        fields[1],
                        fields[2..].iter().copied(),
                    ),
                )))?
            }
        };
        // Each callable occurrence creates its own contract or witness ordinal.
        if let Some(key) = key
            && entered_with == Some(callable_occurrences)
        {
            shared.insert(key, data);
        }
        resolved.push(data);
    }
    resolved.pop()
}

#[cfg(test)]
mod tests;
