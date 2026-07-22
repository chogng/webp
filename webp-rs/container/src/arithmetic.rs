//! Checked RIFF and rectangle arithmetic.

use crate::ContainerError;
use crate::ContainerErrorKind;

pub(crate) fn checked_rect_end(
    origin: u32,
    extent: u32,
    limit: u32,
) -> Result<u32, ContainerError> {
    let end = origin.checked_add(extent).ok_or_else(|| {
        ContainerError::new(
            ContainerErrorKind::InvalidContainer,
            "rectangle end overflow",
        )
    })?;
    if end > limit {
        return Err(ContainerError::new(
            ContainerErrorKind::InvalidContainer,
            "rectangle exceeds containing dimension",
        ));
    }
    Ok(end)
}

pub(crate) fn checked_chunk_end(
    offset: usize,
    payload: u32,
    input_len: usize,
) -> Result<usize, ContainerError> {
    let payload = usize::try_from(payload).map_err(|_| {
        ContainerError::at(
            ContainerErrorKind::InvalidContainer,
            offset,
            "chunk size does not fit usize",
        )
    })?;
    let end = offset
        .checked_add(8)
        .and_then(|value| value.checked_add(payload))
        .and_then(|value| value.checked_add(payload & 1))
        .ok_or_else(|| {
            ContainerError::at(
                ContainerErrorKind::InvalidContainer,
                offset,
                "chunk end overflow",
            )
        })?;
    if end > input_len {
        return Err(ContainerError::at(
            ContainerErrorKind::UnexpectedEof,
            offset,
            "truncated RIFF chunk",
        ));
    }
    Ok(end)
}
