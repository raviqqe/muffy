use crate::literal::Literal;
use muffy_rnc::{DatatypeName, Parameter};
use regex::Regex;

// TODO Resolve datatype prefixes against datatypes declarations.
const XSD_DATATYPE_PREFIX: &str = "xsd";

// Data patterns of string datatypes restricted by single XSD patterns compile
// into pattern literals. All other datatype semantics, like value spaces of
// non-string datatypes or length facets, are approximated as any values.
pub fn resolve_data(name: &DatatypeName, parameters: &[Parameter]) -> Option<Literal> {
    let DatatypeName::Name(name) = name else {
        return None;
    };

    if name.prefix.as_ref().map(ToString::to_string) != Some(XSD_DATATYPE_PREFIX.into())
        || name.local.to_string() != "string"
    {
        return None;
    }

    let patterns = parameters
        .iter()
        .filter(|parameter| {
            parameter.name.prefix.is_none() && parameter.name.local.to_string() == "pattern"
        })
        .collect::<Vec<_>>();
    // XSD conjoins multiple patterns, which a single translated pattern cannot
    // express.
    let [parameter] = patterns.as_slice() else {
        return None;
    };

    // XSD patterns match whole values.
    let pattern = format!(r"\A(?:{})\z", translate_pattern(&parameter.value)?);

    Regex::new(&pattern)
        .is_ok()
        .then_some(Literal::Pattern(pattern))
}

// Translates an XSD pattern into a regular expression, or gives up on
// constructs that have no faithful counterpart.
fn translate_pattern(pattern: &str) -> Option<String> {
    let mut translated = String::new();
    let mut characters = pattern.chars();
    let mut class = false;
    let mut dash = false;

    while let Some(character) = characters.next() {
        let range = dash;
        dash = false;

        match (character, class) {
            ('\\', _) => translate_escape(characters.next()?, class, &mut translated)?,
            ('[', false) => {
                class = true;
                translated.push(character);
            }
            // Nested classes mean class subtraction.
            ('[', true) => return None,
            (']', true) => {
                class = false;
                translated.push(character);
            }
            (']', false) => translated.push_str(r"\]"),
            // A dot matches any character but line breaks.
            ('.', false) => translated.push_str(r"[^\n\r]"),
            // Anchors are ordinary characters.
            ('^' | '$', false) => {
                translated.push('\\');
                translated.push(character);
            }
            // Adjacent dashes mean class difference.
            ('-', true) if range => return None,
            ('-', true) => {
                dash = true;
                translated.push(character);
            }
            // Ampersands and tildes mean class intersection and symmetric
            // difference when doubled.
            ('&' | '~', true) => {
                translated.push('\\');
                translated.push(character);
            }
            _ => translated.push(character),
        }
    }

    (!class).then_some(translated)
}

fn translate_escape(character: char, class: bool, translated: &mut String) -> Option<()> {
    match (character, class) {
        (
            'n' | 'r' | 't' | 'd' | 'D' | '\\' | '|' | '.' | '?' | '*' | '+' | '(' | ')' | '{'
            | '}' | '-' | '[' | ']' | '^',
            _,
        ) => {
            translated.push('\\');
            translated.push(character);
        }
        ('s', false) => translated.push_str(r"[ \t\n\r]"),
        ('S', false) => translated.push_str(r"[^ \t\n\r]"),
        ('s', true) => translated.push_str(r" \t\n\r"),
        _ => return None,
    }

    Some(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use muffy_rnc::{Identifier, Name};

    fn datatype(prefix: &str, local: &str) -> DatatypeName {
        DatatypeName::Name(Name {
            prefix: Some(Identifier {
                component: prefix.into(),
                sub_components: vec![],
            }),
            local: Identifier {
                component: local.into(),
                sub_components: vec![],
            },
        })
    }

    fn parameter(name: &str, value: &str) -> Parameter {
        Parameter {
            name: Name {
                prefix: None,
                local: Identifier {
                    component: name.into(),
                    sub_components: vec![],
                },
            },
            value: value.into(),
        }
    }

    mod resolve {
        use super::*;
        use pretty_assertions::assert_eq;

        #[test]
        fn resolve_pattern() {
            assert_eq!(
                resolve_data(&datatype("xsd", "string"), &[parameter("pattern", "--.*")]),
                Some(Literal::Pattern(r"\A(?:--[^\n\r]*)\z".into()))
            );
        }

        #[test]
        fn resolve_pattern_with_length_parameter() {
            assert_eq!(
                resolve_data(
                    &datatype("xsd", "string"),
                    &[parameter("pattern", "a"), parameter("minLength", "1")]
                ),
                Some(Literal::Pattern(r"\A(?:a)\z".into()))
            );
        }

        #[test]
        fn resolve_no_string_without_parameter() {
            assert_eq!(resolve_data(&datatype("xsd", "string"), &[]), None);
        }

        #[test]
        fn resolve_no_token_datatype() {
            assert_eq!(
                resolve_data(&datatype("xsd", "token"), &[parameter("pattern", "a")]),
                None
            );
        }

        #[test]
        fn resolve_no_unknown_prefix() {
            assert_eq!(
                resolve_data(&datatype("w", "string"), &[parameter("pattern", "a")]),
                None
            );
        }

        #[test]
        fn resolve_no_built_in_datatype() {
            assert_eq!(
                resolve_data(&DatatypeName::String, &[parameter("pattern", "a")]),
                None
            );
        }

        #[test]
        fn resolve_no_multiple_patterns() {
            assert_eq!(
                resolve_data(
                    &datatype("xsd", "string"),
                    &[parameter("pattern", "a"), parameter("pattern", "b")]
                ),
                None
            );
        }

        #[test]
        fn resolve_no_untranslatable_pattern() {
            assert_eq!(
                resolve_data(
                    &datatype("xsd", "string"),
                    &[parameter("pattern", r"[\i-[:]]")]
                ),
                None
            );
        }

        #[test]
        fn resolve_no_invalid_pattern() {
            assert_eq!(
                resolve_data(&datatype("xsd", "string"), &[parameter("pattern", "a)")]),
                None
            );
        }
    }

    mod translate {
        use super::*;
        use pretty_assertions::assert_eq;

        #[test]
        fn translate_characters() {
            assert_eq!(translate_pattern("foo"), Some("foo".into()));
        }

        #[test]
        fn translate_empty_pattern() {
            assert_eq!(translate_pattern(""), Some("".into()));
        }

        #[test]
        fn translate_alternatives_with_quantifiers() {
            assert_eq!(
                translate_pattern("(very){0,2}(thin|thick)+x?"),
                Some("(very){0,2}(thin|thick)+x?".into())
            );
        }

        #[test]
        fn translate_dot() {
            assert_eq!(translate_pattern("."), Some(r"[^\n\r]".into()));
        }

        #[test]
        fn translate_dot_in_class() {
            assert_eq!(translate_pattern("[.]"), Some("[.]".into()));
        }

        #[test]
        fn translate_anchor_characters() {
            assert_eq!(translate_pattern("a^b$"), Some(r"a\^b\$".into()));
        }

        #[test]
        fn translate_class() {
            assert_eq!(
                translate_pattern("[0-9a-fA-F]{6}"),
                Some("[0-9a-fA-F]{6}".into())
            );
        }

        #[test]
        fn translate_negated_class() {
            assert_eq!(translate_pattern("[^ ]+"), Some("[^ ]+".into()));
        }

        #[test]
        fn translate_escaped_characters() {
            assert_eq!(
                translate_pattern(r"\.\{\}\(\)\[\]\-\+\*\?\|\\\^"),
                Some(r"\.\{\}\(\)\[\]\-\+\*\?\|\\\^".into())
            );
        }

        #[test]
        fn translate_control_character_escapes() {
            assert_eq!(translate_pattern(r"\n\r\t"), Some(r"\n\r\t".into()));
        }

        #[test]
        fn translate_digit_escapes() {
            assert_eq!(translate_pattern(r"\d[\d]\D"), Some(r"\d[\d]\D".into()));
        }

        #[test]
        fn translate_whitespace_escapes() {
            assert_eq!(
                translate_pattern(r"\s*\S+"),
                Some(r"[ \t\n\r]*[^ \t\n\r]+".into())
            );
        }

        #[test]
        fn translate_whitespace_escape_in_class() {
            assert_eq!(translate_pattern(r"[x\s]"), Some(r"[x \t\n\r]".into()));
        }

        #[test]
        fn translate_ampersand_and_tilde_in_class() {
            assert_eq!(translate_pattern("[&~]"), Some(r"[\&\~]".into()));
        }

        #[test]
        fn translate_no_name_character_escapes() {
            assert_eq!(translate_pattern(r"\i"), None);
            assert_eq!(translate_pattern(r"\c"), None);
            assert_eq!(translate_pattern(r"\I"), None);
            assert_eq!(translate_pattern(r"\C"), None);
        }

        #[test]
        fn translate_no_word_character_escapes() {
            assert_eq!(translate_pattern(r"\w"), None);
            assert_eq!(translate_pattern(r"\W"), None);
        }

        #[test]
        fn translate_no_category_escape() {
            assert_eq!(translate_pattern(r"\p{Lu}"), None);
        }

        #[test]
        fn translate_no_negated_whitespace_escape_in_class() {
            assert_eq!(translate_pattern(r"[\S]"), None);
        }

        #[test]
        fn translate_no_class_subtraction() {
            assert_eq!(translate_pattern("[a-z-[aeiou]]"), None);
        }

        #[test]
        fn translate_no_class_difference() {
            assert_eq!(translate_pattern("[a-z--a]"), None);
        }

        #[test]
        fn translate_no_trailing_backslash() {
            assert_eq!(translate_pattern("\\"), None);
        }

        #[test]
        fn translate_no_unclosed_class() {
            assert_eq!(translate_pattern("[a"), None);
        }
    }
}
