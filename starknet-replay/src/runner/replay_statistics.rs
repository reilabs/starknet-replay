//! The module which provides an interface to libfunc usage statistics.

use cairo_lang_utils::ordered_hash_map::OrderedHashMap;

#[derive(Debug, Clone, Eq, PartialEq, Default)]
pub struct ReplayStatistics {
    /// This field contains the association between libfunc name (key) and
    /// number of calls (value).
    ///
    /// It is using `OrderedHashMap` because inherited from Cairo crate.
    /// However, there is no architectural reason in `starknet-replay` that
    /// requires it and it can be changed as needed.
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
    /// Keys are added to `self.concrete_libfunc` if not present. If the key
    /// already exists, the value (count) is increased accordingly.
    ///
    /// # Arguments
    ///
    /// - `input`: Input map of libfuncs.
    pub fn add_statistics(&mut self, input: &OrderedHashMap<impl ToString, usize>) {
        for (func_name, weight) in input.iter() {
            self.concrete_libfunc
                .entry(func_name.to_string())
                .and_modify(|e| *e += *weight)
                .or_insert(*weight);
        }
    }

    /// Update `self` with data in `from`.
    ///
    /// This function adopts the same logic as `self.add_statistics`.
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
