//! Legion's M2 models (7.3.5, MD20 version 274), read far enough to draw one in its bind pose: a
//! spike toward later clients' data beside the 1.12.1 base (`docs/EXPANSIONS.md`). Nothing in the
//! client reads it yet.
//!
//! From Legion an `.m2` is a chunked file ([`md21`]): `MD21` wraps the MD20 model, whose offsets
//! count from its own start, and the chunks after it name the files the model needs by file data
//! id: its skin profiles (`SFID`), textures (`TXID`), animations (`AFID`), bones (`BFID`). The
//! MD20 header keeps 1.12.1's walk from Wrath's layout on (no playable animation lookup, the skin
//! profiles out of the file), and its records shrink with the tracks: an `M2Track` is 20 bytes
//! from version 264, its keys an array per sequence ([`model`]). A skin profile is its own `.skin`
//! file ([`skin`]).
//!
//! Which look a creature wears is Legion's DB2 tables' ([`db2`], format WDC1), typed by
//! WoWDBDefs' definitions ([`dbd`], read from a local folder): `CreatureDisplayInfo` and
//! `CreatureModelData` give a display's model, replaceable textures and geosets ([`display`]).

pub mod db2;
pub mod dbd;
pub mod display;
pub mod md21;
pub mod model;
pub mod skin;

use std::fmt;

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    Truncated(&'static str),
    Format(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Truncated(w) => write!(f, "truncated: {w}"),
            Error::Format(w) => write!(f, "format: {w}"),
        }
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

pub(crate) fn u16_at(b: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(at..at + 2)?.try_into().ok()?))
}

pub(crate) fn u32_at(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

pub(crate) fn f32_at(b: &[u8], at: usize) -> Option<f32> {
    u32_at(b, at).map(f32::from_bits)
}

/// An `M2Array` at `at`: `(count, offset)`.
pub(crate) fn array_at(b: &[u8], at: usize) -> Result<(usize, usize)> {
    Ok((
        u32_at(b, at).ok_or(Error::Truncated("array"))? as usize,
        u32_at(b, at + 4).ok_or(Error::Truncated("array"))? as usize,
    ))
}

/// `count` records of `stride` bytes at `offset`, each read by `f`.
pub(crate) fn records<T>(
    b: &[u8],
    (count, offset): (usize, usize),
    stride: usize,
    what: &'static str,
    f: impl Fn(&[u8]) -> Option<T>,
) -> Result<Vec<T>> {
    let end = count
        .checked_mul(stride)
        .and_then(|n| n.checked_add(offset))
        .ok_or(Error::Truncated(what))?;
    let all = b.get(offset..end).ok_or(Error::Truncated(what))?;
    all.chunks_exact(stride)
        .map(|r| f(r).ok_or(Error::Truncated(what)))
        .collect()
}
