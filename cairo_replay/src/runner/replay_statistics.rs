use cairo_lang_utils::ordered_hash_map::OrderedHashMap;
use smol_str::SmolStr;

#[derive(Debug, Clone, Eq, PartialEq, Default)]
pub struct ReplayStatistics {
    pub concrete_libfunc: OrderedHashMap<String, usize>,
}

impl ReplayStatistics {
    pub fn new() -> Self {
        ReplayStatistics {
            concrete_libfunc: OrderedHashMap::default(),
        }
    }

    // TODO: Change in `OrderedHashMap<impl Into<String>, usize>`
    pub fn add_statistics(&mut self, input: &OrderedHashMap<SmolStr, usize>) {
        input.iter().for_each(|(libfunc, weight)| {
            self.concrete_libfunc
                .entry(libfunc.to_string())
                .and_modify(|e| *e += *weight)
                .or_insert(*weight);
        });
    }

    pub fn merge(&mut self, from: &ReplayStatistics) {
        for (libfunc, weight) in from.concrete_libfunc.iter() {
            self.concrete_libfunc
                .entry(libfunc.clone())
                .and_modify(|e| *e += *weight)
                .or_insert(*weight);
        }
    }
}
