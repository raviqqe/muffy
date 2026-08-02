use super::state::State;
use crate::attribute_set::AttributeSet;
use alloc::collections::BTreeSet;

pub struct VariantEvaluation {
    // (error count, conflict count, requirement count)
    pub score: (usize, usize, usize),
    pub attribute_set: &'static AttributeSet,
    pub misplaced_children: BTreeSet<&'static str>,
    pub state: State,
}
