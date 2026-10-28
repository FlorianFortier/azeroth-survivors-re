//! Bands built beside the DBCs' rows, for lights added to the catalog (`super::super::added`).

use super::Band;

impl<T> Band<T> {
    /// A band of `(time, value)` keys, in half-minutes, ascending.
    pub(in crate::light) fn new(keys: Vec<(u32, T)>) -> Self {
        let (times, values) = keys.into_iter().unzip();
        Self { times, values }
    }
}
