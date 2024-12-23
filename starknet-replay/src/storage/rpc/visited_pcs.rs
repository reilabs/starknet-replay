//! This module implements the [`blockifier::state::visited_pcs::VisitedPcs`]
//! trait to allow full record of visited program counters during transaction
//! execution. The default trait used by the blockifier is not enough because it
//! saves all visited program counters in a set.

use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};

use blockifier::state::state_api::State;
use blockifier::state::visited_pcs::VisitedPcs;
use starknet_api::core::ClassHash;

/// The hashmap of [`VisitedPcsRaw`] is a map from a
/// [`starknet_api::core::ClassHash`] to a vector of visited program counters.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct VisitedPcsRaw(pub HashMap<ClassHash, Vec<usize>>);
impl VisitedPcs for VisitedPcsRaw {
    type Pcs = Vec<usize>;

    fn new() -> Self {
        VisitedPcsRaw(HashMap::default())
    }

    fn insert(&mut self, class_hash: &ClassHash, pcs: &[usize]) {
        self.0.entry(*class_hash).or_default().extend(pcs.iter());
    }

    fn iter(&self) -> impl Iterator<Item = (&ClassHash, &Self::Pcs)> {
        self.0.iter()
    }

    fn entry(&mut self, class_hash: ClassHash) -> Entry<'_, ClassHash, Self::Pcs> {
        self.0.entry(class_hash)
    }

    fn extend(&mut self, class_hash: &ClassHash, pcs: &Self::Pcs) {
        self.0.entry(*class_hash).or_default().extend(pcs.clone());
    }

    fn to_set(pcs: Self::Pcs) -> HashSet<usize> {
        let mut set = HashSet::new();
        for p in pcs {
            set.insert(p);
        }
        set
    }

    fn add_visited_pcs(state: &mut dyn State, class_hash: &ClassHash, pcs: Self::Pcs) {
        state.add_visited_pcs(*class_hash, &pcs);
    }
}
