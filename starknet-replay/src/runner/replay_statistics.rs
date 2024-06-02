//! The module which provides an interface to libfunc usage statistics.

use std::ops::{Div, Mul};

use cairo_lang_utils::ordered_hash_map::OrderedHashMap;
use itertools::Itertools;

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
    #[must_use]
    pub fn new() -> Self {
        ReplayStatistics {
            concrete_libfunc: OrderedHashMap::default(),
        }
    }

    /// Add libfunc with frequency to `ReplayStatistics`.
    ///
    /// `name` is added to `self.concrete_libfunc` if not present. If the `name`
    /// already exists, the `frequency` is increased accordingly.
    ///
    /// # Arguments
    ///
    /// - `name`: Name of libfunc.
    /// - `frequency`: Number of calls to `name`.
    pub fn update(&mut self, name: &impl ToString, frequency: usize) {
        self.concrete_libfunc
            .entry(name.to_string())
            .and_modify(|e| *e += frequency)
            .or_insert(frequency);
    }

    /// Update `ReplayStatistics` with results from contract replay.
    ///
    /// Keys are added to `self.concrete_libfunc` if not present. If the key
    /// already exists, the value (count) is increased accordingly.
    ///
    /// # Arguments
    ///
    /// - `input`: Input map of libfuncs.
    #[must_use]
    pub fn add_statistics(mut self, input: &OrderedHashMap<impl ToString, usize>) -> Self {
        for (name, frequency) in input.iter() {
            self.update(&name.to_string(), *frequency);
        }
        self
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
    #[must_use]
    pub fn get_number_of_libfuncs(&self) -> usize {
        self.concrete_libfunc.len()
    }

    /// Returns the number of calls of the most frequently called concrete
    /// libfunc.
    ///
    /// It returns `None` if the map of libfuncs is empty.
    #[must_use]
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
    #[must_use]
    pub fn get_libfunc_frequency(&self, name: &str) -> usize {
        self.concrete_libfunc.get(name).copied().unwrap_or(0)
    }

    /// Filter the most called libfuncs from the set.
    ///
    /// It returns the set of the 80% most called libfuncs ordered from the most
    /// frequent libfunc.
    ///
    /// # Panics
    ///
    /// Panics if the total sum of frequencies doesn't fit in a `usize` number.
    #[must_use]
    pub fn filter_most_frequent(&self) -> ReplayStatistics {
        tracing::info!(
            "Number of libfunc before filtering: {}",
            self.get_number_of_libfuncs()
        );
        let total_libfunc_calls: usize = self.concrete_libfunc.values().sum();
        // Ignoring overflows because the resulting number is less than
        // `total_libfunc_calls`.
        let percentage_of_total: usize = total_libfunc_calls.div(100).mul(80);

        let mut cumulative_frequency: usize = 0;
        let mut truncation_index = self.concrete_libfunc.len();
        for (idx, (_, frequency)) in self
            .concrete_libfunc
            .iter()
            .sorted_by(|a, b| Ord::cmp(&a.1, &b.1))
            .rev()
            .enumerate()
        {
            cumulative_frequency = cumulative_frequency.checked_add(*frequency).unwrap();
            if cumulative_frequency > percentage_of_total {
                truncation_index = idx;
                break;
            }
        }
        let ordered_libfuncs: OrderedHashMap<String, usize> = self
            .concrete_libfunc
            .iter()
            .sorted_by(|a, b| Ord::cmp(&a.1, &b.1))
            .rev()
            .take(truncation_index)
            .map(|(name, freq)| (name.clone(), *freq))
            .collect();
        let filtered_libfuncs = ReplayStatistics::default().add_statistics(&ordered_libfuncs);
        tracing::info!(
            "Number of libfunc before filtering: {}",
            filtered_libfuncs.get_number_of_libfuncs()
        );
        filtered_libfuncs
    }
}
