use alloc::collections::BTreeMap;
use muffy_rnc::{
    Declaration, Grammar, GrammarContent, Identifier, Include, Name, NameClass, Pattern, Schema,
    SchemaBody,
};

// Canonical prefixes of namespace URIs consistent with document name
// canonicalization in the document crate.
const CANONICAL_PREFIXES: &[(&str, &str)] = &[
    ("http://creativecommons.org/ns#", "cc"),
    ("http://purl.org/dc/elements/1.1/", "dc"),
    (
        "http://sodipodi.sourceforge.net/DTD/sodipodi-0.dtd",
        "sodipodi",
    ),
    ("http://www.inkscape.org/namespaces/inkscape", "inkscape"),
    ("http://www.w3.org/1999/02/22-rdf-syntax-ns#", "rdf"),
    ("http://www.w3.org/1999/xlink", "xlink"),
    ("http://www.w3.org/XML/1998/namespace", "xml"),
];

// Resolves prefixes in element and attribute names into namespace URIs
// declared in a schema, and renders the names with canonical prefixes.
// Prefixes of undeclared or unrecognized namespaces are kept as they are.
pub fn resolve_namespaces(schema: Schema) -> Schema {
    let namespaces = schema
        .declarations
        .iter()
        .filter_map(|declaration| match declaration {
            Declaration::Namespace(declaration) => {
                Some((declaration.prefix.clone(), declaration.uri.clone()))
            }
            Declaration::Datatypes(_) | Declaration::DefaultNamespace(_) => None,
        })
        .collect();

    Schema {
        body: match schema.body {
            SchemaBody::Grammar(grammar) => {
                SchemaBody::Grammar(resolve_grammar(grammar, &namespaces))
            }
            SchemaBody::Pattern(pattern) => {
                SchemaBody::Pattern(resolve_pattern(pattern, &namespaces))
            }
        },
        declarations: schema.declarations,
    }
}

fn resolve_grammar(grammar: Grammar, namespaces: &BTreeMap<Identifier, String>) -> Grammar {
    Grammar {
        contents: grammar
            .contents
            .into_iter()
            .map(|content| match content {
                GrammarContent::Annotation(_) => content,
                GrammarContent::Definition(definition) => {
                    GrammarContent::Definition(muffy_rnc::Definition {
                        pattern: resolve_pattern(definition.pattern, namespaces),
                        ..definition
                    })
                }
                GrammarContent::Div(grammar) => {
                    GrammarContent::Div(resolve_grammar(grammar, namespaces))
                }
                GrammarContent::Include(include) => GrammarContent::Include(Include {
                    grammar: include
                        .grammar
                        .map(|grammar| resolve_grammar(grammar, namespaces)),
                    ..include
                }),
                GrammarContent::Start { combine, pattern } => GrammarContent::Start {
                    combine,
                    pattern: resolve_pattern(pattern, namespaces),
                },
            })
            .collect(),
    }
}

fn resolve_pattern(pattern: Pattern, namespaces: &BTreeMap<Identifier, String>) -> Pattern {
    match pattern {
        Pattern::Attribute {
            name_class,
            pattern,
        } => Pattern::Attribute {
            name_class: resolve_name_class(name_class, namespaces),
            pattern: resolve_pattern(*pattern, namespaces).into(),
        },
        Pattern::Choice(patterns) => Pattern::Choice(
            patterns
                .into_iter()
                .map(|pattern| resolve_pattern(pattern, namespaces))
                .collect(),
        ),
        Pattern::Data {
            name,
            parameters,
            except,
        } => Pattern::Data {
            name,
            parameters,
            except: except.map(|pattern| resolve_pattern(*pattern, namespaces).into()),
        },
        Pattern::Element {
            name_class,
            pattern,
        } => Pattern::Element {
            name_class: resolve_name_class(name_class, namespaces),
            pattern: resolve_pattern(*pattern, namespaces).into(),
        },
        Pattern::Grammar(grammar) => Pattern::Grammar(resolve_grammar(grammar, namespaces)),
        Pattern::Group(patterns) => Pattern::Group(
            patterns
                .into_iter()
                .map(|pattern| resolve_pattern(pattern, namespaces))
                .collect(),
        ),
        Pattern::Interleave(patterns) => Pattern::Interleave(
            patterns
                .into_iter()
                .map(|pattern| resolve_pattern(pattern, namespaces))
                .collect(),
        ),
        Pattern::List(pattern) => Pattern::List(resolve_pattern(*pattern, namespaces).into()),
        Pattern::Many0(pattern) => Pattern::Many0(resolve_pattern(*pattern, namespaces).into()),
        Pattern::Many1(pattern) => Pattern::Many1(resolve_pattern(*pattern, namespaces).into()),
        Pattern::Optional(pattern) => {
            Pattern::Optional(resolve_pattern(*pattern, namespaces).into())
        }
        Pattern::Empty
        | Pattern::External(_)
        | Pattern::Name(_)
        | Pattern::NotAllowed
        | Pattern::Text
        | Pattern::Value { .. } => pattern,
    }
}

fn resolve_name_class(
    name_class: NameClass,
    namespaces: &BTreeMap<Identifier, String>,
) -> NameClass {
    match name_class {
        NameClass::AnyName => NameClass::AnyName,
        NameClass::Choice(classes) => NameClass::Choice(
            classes
                .into_iter()
                .map(|class| resolve_name_class(class, namespaces))
                .collect(),
        ),
        NameClass::Except { base, except } => NameClass::Except {
            base: resolve_name_class(*base, namespaces).into(),
            except: resolve_name_class(*except, namespaces).into(),
        },
        NameClass::Name(name) => NameClass::Name(resolve_name(name, namespaces)),
        NameClass::NamespaceName(Some(prefix)) => match namespaces.get(&prefix) {
            Some(uri) if uri.is_empty() => NameClass::NamespaceName(None),
            Some(uri) => NameClass::NamespaceName(Some(canonical_prefix(uri).unwrap_or(prefix))),
            None => NameClass::NamespaceName(Some(prefix)),
        },
        NameClass::NamespaceName(None) => NameClass::NamespaceName(None),
    }
}

fn resolve_name(name: Name, namespaces: &BTreeMap<Identifier, String>) -> Name {
    let Some(uri) = name
        .prefix
        .as_ref()
        .and_then(|prefix| namespaces.get(prefix))
    else {
        return name;
    };

    if uri.is_empty() {
        Name {
            prefix: None,
            local: name.local,
        }
    } else if let Some(prefix) = canonical_prefix(uri) {
        Name {
            prefix: Some(prefix),
            local: name.local,
        }
    } else {
        name
    }
}

fn canonical_prefix(uri: &str) -> Option<Identifier> {
    CANONICAL_PREFIXES.iter().find_map(|(candidate, prefix)| {
        (*candidate == uri).then(|| Identifier {
            component: (*prefix).to_owned(),
            sub_components: vec![],
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use muffy_rnc::parse_schema;
    use pretty_assertions::assert_eq;

    fn resolve(source: &str) -> Pattern {
        let SchemaBody::Grammar(grammar) = resolve_namespaces(parse_schema(source).unwrap()).body
        else {
            panic!("grammar expected");
        };
        let GrammarContent::Definition(definition) = &grammar.contents[0] else {
            panic!("definition expected");
        };

        definition.pattern.clone()
    }

    fn attribute_name_class(pattern: &Pattern) -> &NameClass {
        let Pattern::Attribute { name_class, .. } = pattern else {
            panic!("attribute expected");
        };

        name_class
    }

    #[test]
    fn keep_undeclared_prefix() {
        assert_eq!(
            attribute_name_class(&resolve("root = attribute foo:bar { text }")),
            &NameClass::Name(Name {
                prefix: Some(Identifier {
                    component: "foo".into(),
                    sub_components: vec![],
                }),
                local: Identifier {
                    component: "bar".into(),
                    sub_components: vec![],
                },
            })
        );
    }

    #[test]
    fn keep_prefix_of_unrecognized_namespace() {
        assert_eq!(
            attribute_name_class(&resolve(
                "namespace foo = \"http://foo.example/\"\nroot = attribute foo:bar { text }"
            )),
            &NameClass::Name(Name {
                prefix: Some(Identifier {
                    component: "foo".into(),
                    sub_components: vec![],
                }),
                local: Identifier {
                    component: "bar".into(),
                    sub_components: vec![],
                },
            })
        );
    }

    #[test]
    fn resolve_name_in_empty_namespace() {
        assert_eq!(
            attribute_name_class(&resolve(
                "namespace none = \"\"\nroot = attribute none:foo { text }"
            )),
            &NameClass::Name(Name {
                prefix: None,
                local: Identifier {
                    component: "foo".into(),
                    sub_components: vec![],
                },
            })
        );
    }

    #[test]
    fn resolve_wildcard_in_empty_namespace() {
        assert_eq!(
            attribute_name_class(&resolve(
                "namespace none = \"\"\nroot = attribute none:* { text }"
            )),
            &NameClass::NamespaceName(None)
        );
    }

    #[test]
    fn resolve_canonical_prefix_of_recognized_namespace() {
        assert_eq!(
            attribute_name_class(&resolve(
                "namespace ink = \"http://www.inkscape.org/namespaces/inkscape\"\nroot = attribute ink:* { text }"
            )),
            &NameClass::NamespaceName(Some(Identifier {
                component: "inkscape".into(),
                sub_components: vec![],
            }))
        );
    }
}
