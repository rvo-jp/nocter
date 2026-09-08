use std::fmt;

/// One byte range placed by the common target object-layout authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Arm64ObjectRange {
    offset: u64,
    size: u64,
    alignment: u64,
}

impl Arm64ObjectRange {
    pub(crate) const fn offset(self) -> u64 {
        self.offset
    }

    pub(crate) const fn size(self) -> u64 {
        self.size
    }

    pub(crate) const fn alignment(self) -> u64 {
        self.alignment
    }
}

/// Deterministic sequential placement shared by stack frames and allocation-backed async frames.
pub(crate) struct Arm64ObjectSequence {
    next: u64,
    alignment: u64,
    maximum_alignment: u64,
}

impl Arm64ObjectSequence {
    pub(crate) const fn new(start: u64, minimum_alignment: u64, maximum_alignment: u64) -> Self {
        Self {
            next: start,
            alignment: minimum_alignment,
            maximum_alignment,
        }
    }

    pub(crate) fn add(
        &mut self,
        size: u64,
        alignment: u64,
    ) -> Result<Arm64ObjectRange, Arm64ObjectLayoutError> {
        validate_alignment(alignment, self.maximum_alignment)?;
        let offset = align_up(self.next, alignment)?;
        self.next = offset
            .checked_add(size)
            .ok_or(Arm64ObjectLayoutError::SizeOverflow)?;
        self.alignment = self.alignment.max(alignment);
        Ok(Arm64ObjectRange {
            offset,
            size,
            alignment,
        })
    }

    pub(crate) fn finish(self, final_alignment: u64) -> Result<(u64, u64), Arm64ObjectLayoutError> {
        validate_alignment(final_alignment, self.maximum_alignment)?;
        Ok((
            align_up(self.next, final_alignment.max(self.alignment))?,
            self.alignment,
        ))
    }

    pub(crate) fn finish_with_trailer(
        self,
        trailer_size: u64,
        final_alignment: u64,
    ) -> Result<(u64, u64, u64), Arm64ObjectLayoutError> {
        validate_alignment(final_alignment, self.maximum_alignment)?;
        let size = align_up(
            self.next
                .checked_add(trailer_size)
                .ok_or(Arm64ObjectLayoutError::SizeOverflow)?,
            final_alignment.max(self.alignment),
        )?;
        Ok((size, size - trailer_size, self.alignment))
    }
}

pub(crate) fn validate_alignment(
    alignment: u64,
    maximum_alignment: u64,
) -> Result<(), Arm64ObjectLayoutError> {
    if !alignment.is_power_of_two() || alignment > maximum_alignment {
        return Err(Arm64ObjectLayoutError::InvalidAlignment(alignment));
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Arm64ObjectLayoutError {
    InvalidAlignment(u64),
    SizeOverflow,
}

impl fmt::Display for Arm64ObjectLayoutError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "ARM64 object layout failed: {self:?}")
    }
}

impl std::error::Error for Arm64ObjectLayoutError {}

fn align_up(value: u64, alignment: u64) -> Result<u64, Arm64ObjectLayoutError> {
    let mask = alignment - 1;
    value
        .checked_add(mask)
        .map(|value| value & !mask)
        .ok_or(Arm64ObjectLayoutError::SizeOverflow)
}
