use super::*;

#[derive(Debug, Clone, Default)]
pub(in crate::typecheck::returns) struct BorrowReturnEnvironment {
    bindings: HashMap<String, BorrowReturnProvenance>,
}

pub(in crate::typecheck::returns) type BorrowReturnSummaries =
    HashMap<ByteSpan, BorrowReturnProvenance>;

#[derive(Debug, Clone, Default)]
pub(in crate::typecheck::returns) struct BorrowReturnFlow {
    value: Option<BorrowReturnProvenance>,
    fallible_error: Option<BorrowReturnProvenance>,
}

impl BorrowReturnFlow {
    pub(in crate::typecheck::returns) fn merge_value(
        &mut self,
        provenance: Option<BorrowReturnProvenance>,
    ) {
        merge_borrow_return_provenance(&mut self.value, provenance);
    }

    pub(in crate::typecheck::returns) fn merge_fallible_error(
        &mut self,
        provenance: Option<BorrowReturnProvenance>,
    ) {
        merge_borrow_return_provenance(&mut self.fallible_error, provenance);
    }

    pub(in crate::typecheck::returns) fn into_return_provenance(
        self,
        return_type: &Type,
    ) -> Option<BorrowReturnProvenance> {
        if matches!(return_type, Type::Fallible { .. }) {
            return borrow_return_fallible_provenance(self.value, self.fallible_error);
        }

        self.value
    }
}

impl BorrowReturnEnvironment {
    pub(in crate::typecheck::returns) fn get(&self, name: &str) -> Option<&BorrowReturnProvenance> {
        self.bindings.get(name)
    }

    pub(in crate::typecheck::returns) fn define_binding(
        &mut self,
        name: String,
        contains_borrow_like: bool,
        provenance: Option<BorrowReturnProvenance>,
    ) {
        if contains_borrow_like {
            if let Some(provenance) = provenance {
                self.bindings.insert(name, provenance);
            } else {
                self.bindings.remove(&name);
            }
        } else {
            self.bindings.remove(&name);
        }
    }

    pub(in crate::typecheck::returns) fn join_reachable(
        &mut self,
        states: &[BorrowReturnEnvironment],
    ) {
        let mut joined = HashMap::new();
        for state in states {
            for (name, provenance) in &state.bindings {
                joined
                    .entry(name.clone())
                    .and_modify(|existing: &mut BorrowReturnProvenance| existing.merge(provenance))
                    .or_insert_with(|| provenance.clone());
            }
        }
        self.bindings = joined;
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::typecheck::returns) enum BorrowReturnProvenance {
    Static,
    InputBorrow {
        sources: BTreeSet<String>,
    },
    Escaping {
        source: String,
    },
    Aggregate {
        fallback: Option<Box<BorrowReturnProvenance>>,
        fields: BTreeMap<String, BorrowReturnProvenance>,
        elements: BTreeMap<usize, BorrowReturnProvenance>,
    },
    Fallible {
        success: Option<Box<BorrowReturnProvenance>>,
        error: Option<Box<BorrowReturnProvenance>>,
    },
}

impl BorrowReturnProvenance {
    pub(in crate::typecheck::returns) fn input_borrow(source: String) -> Self {
        Self::InputBorrow {
            sources: BTreeSet::from([source]),
        }
    }

    pub(in crate::typecheck::returns) fn escaping(source: String) -> Self {
        Self::Escaping { source }
    }

    pub(in crate::typecheck::returns) fn escaping_source(&self) -> Option<&str> {
        match self {
            Self::Escaping { source } => Some(source),
            Self::Aggregate {
                fallback,
                fields,
                elements,
            } => fallback
                .as_deref()
                .and_then(BorrowReturnProvenance::escaping_source)
                .or_else(|| {
                    fields
                        .values()
                        .find_map(BorrowReturnProvenance::escaping_source)
                })
                .or_else(|| {
                    elements
                        .values()
                        .find_map(BorrowReturnProvenance::escaping_source)
                }),
            Self::Fallible { success, error } => success
                .as_deref()
                .and_then(BorrowReturnProvenance::escaping_source)
                .or_else(|| {
                    error
                        .as_deref()
                        .and_then(BorrowReturnProvenance::escaping_source)
                }),
            Self::Static | Self::InputBorrow { .. } => None,
        }
    }

    pub(in crate::typecheck::returns) fn success_provenance(
        &self,
    ) -> Option<BorrowReturnProvenance> {
        match self {
            Self::Fallible { success, .. } => success.as_deref().cloned(),
            _ => Some(self.clone()),
        }
    }

    pub(in crate::typecheck::returns) fn fallible_error_provenance(
        &self,
    ) -> Option<BorrowReturnProvenance> {
        match self {
            Self::Fallible { error, .. } => error.as_deref().cloned(),
            _ => None,
        }
    }

    pub(in crate::typecheck::returns) fn field_provenance(
        &self,
        field: &str,
    ) -> Option<BorrowReturnProvenance> {
        match self {
            Self::Aggregate {
                fallback, fields, ..
            } => {
                let mut provenance = fallback.as_deref().cloned();
                merge_borrow_return_provenance(&mut provenance, fields.get(field).cloned());
                provenance
            }
            _ => Some(self.clone()),
        }
    }

    pub(in crate::typecheck::returns) fn element_provenance(
        &self,
        index: Option<usize>,
    ) -> Option<BorrowReturnProvenance> {
        match self {
            Self::Aggregate {
                fallback, elements, ..
            } => {
                let mut provenance = fallback.as_deref().cloned();
                if let Some(index) = index {
                    merge_borrow_return_provenance(&mut provenance, elements.get(&index).cloned());
                } else {
                    for element_provenance in elements.values() {
                        merge_borrow_return_provenance(
                            &mut provenance,
                            Some(element_provenance.clone()),
                        );
                    }
                }
                provenance
            }
            _ => Some(self.clone()),
        }
    }

    pub(in crate::typecheck::returns) fn merge(&mut self, other: &BorrowReturnProvenance) {
        match (&mut *self, other) {
            (Self::Escaping { .. }, _) => {}
            (_, Self::Escaping { source }) => {
                *self = Self::Escaping {
                    source: source.clone(),
                };
            }
            (
                Self::Aggregate {
                    fallback,
                    fields,
                    elements,
                },
                Self::Aggregate {
                    fallback: other_fallback,
                    fields: other_fields,
                    elements: other_elements,
                },
            ) => {
                merge_borrow_return_boxed_provenance(fallback, other_fallback.as_deref().cloned());
                for (field, other_field_provenance) in other_fields {
                    fields
                        .entry(field.clone())
                        .and_modify(|field_provenance| {
                            field_provenance.merge(other_field_provenance)
                        })
                        .or_insert_with(|| other_field_provenance.clone());
                }
                for (index, other_element_provenance) in other_elements {
                    elements
                        .entry(*index)
                        .and_modify(|element_provenance| {
                            element_provenance.merge(other_element_provenance)
                        })
                        .or_insert_with(|| other_element_provenance.clone());
                }
            }
            (
                Self::Fallible { success, error },
                Self::Fallible {
                    success: other_success,
                    error: other_error,
                },
            ) => {
                merge_borrow_return_boxed_provenance(success, other_success.as_deref().cloned());
                merge_borrow_return_boxed_provenance(error, other_error.as_deref().cloned());
            }
            (Self::Fallible { success, .. }, other) => {
                merge_borrow_return_boxed_provenance(success, Some(other.clone()));
            }
            (existing, Self::Fallible { success, error }) => {
                let mut merged_success = success.as_deref().cloned();
                merge_borrow_return_provenance(&mut merged_success, Some(existing.clone()));
                *existing = Self::Fallible {
                    success: merged_success.map(Box::new),
                    error: error.clone(),
                };
            }
            (
                Self::Aggregate {
                    fallback,
                    fields: _,
                    elements: _,
                },
                other,
            ) => {
                merge_borrow_return_boxed_provenance(fallback, Some(other.clone()));
            }
            (
                existing,
                Self::Aggregate {
                    fallback,
                    fields,
                    elements,
                },
            ) => {
                let mut merged_fallback = fallback.as_deref().cloned();
                merge_borrow_return_provenance(&mut merged_fallback, Some(existing.clone()));
                *existing = Self::Aggregate {
                    fallback: merged_fallback.map(Box::new),
                    fields: fields.clone(),
                    elements: elements.clone(),
                };
            }
            (
                Self::InputBorrow { sources },
                Self::InputBorrow {
                    sources: other_sources,
                },
            ) => {
                sources.extend(other_sources.iter().cloned());
            }
            (Self::Static, Self::InputBorrow { sources }) => {
                *self = Self::InputBorrow {
                    sources: sources.clone(),
                };
            }
            (Self::InputBorrow { .. }, Self::Static) | (Self::Static, Self::Static) => {}
        }
    }
}

pub(in crate::typecheck::returns) fn borrow_return_fallible_provenance(
    success: Option<BorrowReturnProvenance>,
    error: Option<BorrowReturnProvenance>,
) -> Option<BorrowReturnProvenance> {
    if success.is_none() && error.is_none() {
        return None;
    }

    Some(BorrowReturnProvenance::Fallible {
        success: success.map(Box::new),
        error: error.map(Box::new),
    })
}

pub(in crate::typecheck::returns) fn merge_borrow_return_provenance(
    provenance: &mut Option<BorrowReturnProvenance>,
    next: Option<BorrowReturnProvenance>,
) {
    let Some(next) = next else {
        return;
    };
    if let Some(existing) = provenance {
        existing.merge(&next);
    } else {
        *provenance = Some(next);
    }
}

pub(in crate::typecheck::returns) fn merge_borrow_return_boxed_provenance(
    provenance: &mut Option<Box<BorrowReturnProvenance>>,
    next: Option<BorrowReturnProvenance>,
) {
    let mut unboxed = provenance.take().map(|provenance| *provenance);
    merge_borrow_return_provenance(&mut unboxed, next);
    *provenance = unboxed.map(Box::new);
}
