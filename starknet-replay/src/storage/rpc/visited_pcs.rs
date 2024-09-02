use std::collections::hash_map::{Entry, IntoIter, Iter};
use std::collections::{HashMap, HashSet};

use blockifier::state::state_api::State;
use blockifier::state::visited_pcs::VisitedPcs;
use starknet_api::core::ClassHash;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct VisitedPcsRaw(HashMap<ClassHash, Vec<Vec<usize>>>);
impl VisitedPcsRaw {
    pub fn iter(&self) -> impl Iterator<Item = (&ClassHash, &Vec<Vec<usize>>)> {
        self.into_iter()
    }
}
impl VisitedPcs for VisitedPcsRaw {
    type Pcs = Vec<Vec<usize>>;

    fn new() -> Self {
        VisitedPcsRaw(HashMap::default())
    }

    fn insert(&mut self, class_hash: &ClassHash, pcs: &[usize]) {
        self.0.entry(*class_hash).or_default().push(pcs.to_vec());
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
        pcs.into_iter().flatten().for_each(|p| {
            set.insert(p);
        });
        set
    }

    fn add_visited_pcs(state: &mut dyn State, class_hash: &ClassHash, pcs: Self::Pcs) {
        for pc in pcs {
            state.add_visited_pcs(*class_hash, &pc);
        }
    }
}
impl IntoIterator for VisitedPcsRaw {
    type Item = (ClassHash, Vec<Vec<usize>>);
    type IntoIter = IntoIter<ClassHash, Vec<Vec<usize>>>;

    fn into_iter(self) -> IntoIter<ClassHash, Vec<Vec<usize>>> {
        self.0.into_iter()
    }
}
impl<'a> IntoIterator for &'a VisitedPcsRaw {
    type Item = (&'a ClassHash, &'a Vec<Vec<usize>>);
    type IntoIter = Iter<'a, ClassHash, Vec<Vec<usize>>>;

    fn into_iter(self) -> Iter<'a, ClassHash, Vec<Vec<usize>>> {
        self.0.iter()
    }
}
