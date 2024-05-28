//! The module which provides an interface to libfunc usage statistics.

use cairo_lang_utils::ordered_hash_map::OrderedHashMap;
use smol_str::SmolStr;

#[derive(Debug, Clone, Eq, PartialEq, Default)]
pub struct ReplayStatistics {
    /// This field contains the association between libfunc name (key) and
    /// number of calls (value).
    pub concrete_libfunc: OrderedHashMap<String, usize>,
}

impl ReplayStatistics {
    /// Initialisation of `ReplayStatistics`.
    ///
    /// The struct is initialised with field `concrete_libfunc` empty.
    pub fn new() -> Self {
        ReplayStatistics {
            concrete_libfunc: OrderedHashMap::default(),
        }
    }

    /// Update `ReplayStatistics` with results from contract replay.
    ///
    /// Keys are added if new. If keys exist already, the value (count) is
    /// increased accordingly.
    ///
    /// # Arguments
    ///
    /// - `input`: Input map of libfuncs.
    // TODO: Change in `OrderedHashMap<impl Into<String>, usize>`
    pub fn add_statistics(&mut self, input: &OrderedHashMap<SmolStr, usize>) {
        input.iter().for_each(|(libfunc, weight)| {
            self.concrete_libfunc
                .entry(libfunc.to_string())
                .and_modify(|e| *e += *weight)
                .or_insert(*weight);
        });
    }

    /// Update `self` with data in `from`.
    ///
    /// Same logic as for `self.add_statistics`.
    ///
    /// # Arguments
    ///
    /// - `from`: Input `ReplayStatistics` to get data from.
    pub fn merge(&mut self, from: &ReplayStatistics) {
        for (libfunc, weight) in from.concrete_libfunc.iter() {
            self.concrete_libfunc
                .entry(libfunc.clone())
                .and_modify(|e| *e += *weight)
                .or_insert(*weight);
        }
    }
}
