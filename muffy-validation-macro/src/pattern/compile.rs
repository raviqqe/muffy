use super::{resolved_pattern::ResolvedPattern, split::split_pattern};
use crate::error::MacroError;
use alloc::collections::BTreeMap;
use muffy_rnc::{Identifier, Pattern};

pub struct Compiler<'a> {
    pub definitions: &'a BTreeMap<Identifier, Pattern>,
    pub cache: BTreeMap<Identifier, ResolvedPattern>,
}

impl<'a> Compiler<'a> {
    pub fn new(definitions: &'a BTreeMap<Identifier, Pattern>) -> Self {
        Self {
            definitions,
            cache: Default::default(),
        }
    }

    pub fn compile(
        &mut self,
        pattern: &Pattern,
    ) -> Result<Vec<(ResolvedPattern, ResolvedPattern)>, MacroError> {
        split_pattern(&self.resolve(pattern)?)
    }
}
