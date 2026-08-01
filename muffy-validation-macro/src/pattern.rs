mod resolve;
mod resolved_pattern;
mod split;

pub use self::resolved_pattern::ResolvedPattern;
use self::{resolve::resolve_pattern, split::split_pattern};
use crate::error::MacroError;
use alloc::collections::{BTreeMap, BTreeSet};
use muffy_rnc::{Identifier, NameClass, Pattern};

pub fn compile_pattern(
    pattern: &Pattern,
    definitions: &BTreeMap<Identifier, Pattern>,
    cache: &mut BTreeMap<Identifier, ResolvedPattern>,
) -> Result<Vec<(ResolvedPattern, ResolvedPattern)>, MacroError> {
    split_pattern(&resolve_pattern(pattern, definitions, cache)?)
}

pub fn class_names(name_class: &NameClass) -> BTreeSet<String> {
    match name_class {
        NameClass::Name(name) => [identifier_string(&name.local)].into(),
        NameClass::Choice(classes) => classes.iter().flat_map(class_names).collect(),
        // TODO Support wildcard name classes (e.g. custom elements).
        NameClass::AnyName | NameClass::Except { .. } | NameClass::NamespaceName(_) => {
            Default::default()
        }
    }
}

fn identifier_string(identifier: &Identifier) -> String {
    identifier
        .sub_components
        .iter()
        .fold(identifier.component.clone(), |string, component| {
            string + "." + component
        })
}
