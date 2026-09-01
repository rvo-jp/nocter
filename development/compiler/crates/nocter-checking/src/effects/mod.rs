mod analysis;
mod table;

use std::collections::HashMap;

use nocter_model::{BodyId, BodyNodeId};
use nocter_source_index::SourceOrigin;

use crate::{BodyCheckError, BodySource, CheckedBody, ClosureTable};

pub use table::{AllocationEffect, EffectTable};

pub(crate) struct EffectBodyInput<'program, 'syntax> {
    source: BodySource<'syntax>,
    body: &'program CheckedBody,
    origins: &'program HashMap<BodyNodeId, SourceOrigin>,
}

impl<'program, 'syntax> EffectBodyInput<'program, 'syntax> {
    pub(crate) const fn new(
        source: BodySource<'syntax>,
        body: &'program CheckedBody,
        origins: &'program HashMap<BodyNodeId, SourceOrigin>,
    ) -> Self {
        Self {
            source,
            body,
            origins,
        }
    }

    const fn source(&self) -> BodySource<'syntax> {
        self.source
    }

    const fn body(&self) -> &'program CheckedBody {
        self.body
    }

    const fn origins(&self) -> &'program HashMap<BodyNodeId, SourceOrigin> {
        self.origins
    }
}

pub(crate) fn analyze_program_effects(
    environment: &crate::program_environment::ProgramEnvironment,
    closures: &ClosureTable,
    inputs: &[EffectBodyInput<'_, '_>],
) -> Result<EffectTable, BodyCheckError> {
    analysis::analyze_program(environment, closures, inputs)
}

fn input_for_body<'a, 'syntax>(
    inputs: &'a [EffectBodyInput<'a, 'syntax>],
    body: BodyId,
) -> Option<&'a EffectBodyInput<'a, 'syntax>> {
    inputs.iter().find(|input| input.source().body() == body)
}
