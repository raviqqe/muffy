use super::{resolve::resolve_pattern, resolved_pattern::ResolvedPattern, split::split_pattern};
use crate::error::MacroError;
use alloc::collections::BTreeMap;
use muffy_rnc::{Identifier, Pattern};

pub struct Compiler<'a> {
    definitions: &'a BTreeMap<Identifier, Pattern>,
    cache: BTreeMap<Identifier, ResolvedPattern>,
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
        split_pattern(&resolve_pattern(
            pattern,
            self.definitions,
            &mut self.cache,
        )?)
    }
}
