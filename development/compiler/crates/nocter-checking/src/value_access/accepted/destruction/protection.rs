//! A scoped edit may update the receiver's fields while its own retirement is in progress.
//! Reusing the original whole referent for another retirement would run the body recursively.
use crate::value_access::{Argument, ArgumentAction, Query, Referent, flow, paths::ProjectionRoot};
use nocter_model::AccessLocalId;
use std::collections::BTreeSet;

const UNKNOWN: &str = "this operation has no verified receiver origin during destruction";

pub(super) struct Protected<'a> {
    typed: &'a Query,
    locals: &'a BTreeSet<AccessLocalId>,
    aliases: Option<flow::Aliases>,
}

impl<'a> Protected<'a> {
    pub(super) fn new(
        typed: &'a Query,
        locals: &'a BTreeSet<AccessLocalId>,
    ) -> Result<Self, flow::IntegrityError> {
        let aliases = (!locals.is_empty())
            .then(|| flow::Aliases::prepare(typed))
            .transpose()?;
        Ok(Self {
            typed,
            locals,
            aliases,
        })
    }

    pub(super) fn check_replacement(&self, source: Referent) -> Result<(), &'static str> {
        if self.contains(source)? {
            return Err("cannot replace the value whose destructor is running");
        }
        Ok(())
    }

    pub(super) fn positions(&self, arguments: &[Argument]) -> Result<Vec<usize>, &'static str> {
        let mut positions = Vec::new();
        for (index, argument) in arguments.iter().enumerate() {
            if let ArgumentAction::ReserveUpdate(source) = argument.action
                && self.contains(source)?
            {
                positions.push(index);
            }
        }
        Ok(positions)
    }

    fn contains(&self, source: Referent) -> Result<bool, &'static str> {
        let Some(aliases) = &self.aliases else {
            return Ok(false);
        };
        let body = self.typed.body();
        let data = body.referent_data(source).ok_or(UNKNOWN)?;
        if !self.locals.iter().any(|&local| {
            body.locals()
                .get(local)
                .is_some_and(|local| local.data == data)
        }) {
            return Ok(false);
        }
        let origins = aliases.producer_paths(self.typed, source).ok_or(UNKNOWN)?;
        for (root, path) in origins {
            if let ProjectionRoot::Place(place) = root
                && path.is_root()
                && self
                    .locals
                    .contains(&body.places().get(place).ok_or(UNKNOWN)?.local)
            {
                return Ok(true);
            }
        }
        Ok(false)
    }
}
