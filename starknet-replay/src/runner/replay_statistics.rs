//! The module which provides an interface to libfunc usage statistics.

use cairo_lang_utils::ordered_hash_map::OrderedHashMap;

/// The struct to hold a list of libfunc names with their related calling
/// frequency.
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

    /// Returns the number of different concrete libfunc names in the
    /// `ReplayStatistics` object.
    pub fn get_number_of_libfuncs(&self) -> usize {
        self.concrete_libfunc.len()
    }

    /// Returns the number of calls of the most frequently called concrete
    /// libfunc.
    ///
    /// It returns `None` if the map of libfuncs is empty.
    pub fn get_highest_frequency(&self) -> Option<usize> {
        self.concrete_libfunc.values().max().copied()
    }

    /// Returns the vector of the concrete libfunc names without their
    /// frequency.
    pub fn get_libfuncs(&self) -> Vec<&str> {
        self.concrete_libfunc
            .keys()
            .map(std::string::String::as_str)
            .collect::<Vec<&str>>()
    }

    /// Queries the frequency of the a given concrete libfunc name.
    ///
    /// If the libfunc isn't found, it returns 0.
    ///
    /// # Arguments
    ///
    /// - `name`: The libfunc to query.
    pub fn get_libfunc_frequency(&self, name: &str) -> usize {
        self.concrete_libfunc.get(name).copied().unwrap_or(0)
    }
}