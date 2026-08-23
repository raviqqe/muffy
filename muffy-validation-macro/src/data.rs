mod error;

pub use self::error::XsdPatternError;
use crate::{error::MacroError, literal::Literal};
use muffy_rnc::{DatatypeName, Parameter};
use regex::Regex;

// TODO Resolve datatype prefixes against datatypes declarations.
const XSD_DATATYPE_PREFIX: &str = "xsd";

// Data patterns of string datatypes restricted by single XSD patterns compile
// into pattern literals. Invalid XSD patterns are errors while untranslatable
// ones leave values unrestricted.
//
// TODO Validate other datatype semantics, like value spaces of non-string
// datatypes and length facets.
pub fn resolve_data(
    name: &DatatypeName,
    parameters: &[Parameter],
) -> Result<Option<Literal>, MacroError> {
    let DatatypeName::Name(name) = name else {
        return Ok(None);
    };

    if name.prefix.as_ref().map(ToString::to_string) != Some(XSD_DATATYPE_PREFIX.into())
        || name.local.to_string() != "string"
    {
        return Ok(None);
    }

    let patterns = parameters
        .iter()
        .filter(|parameter| {
            parameter.name.prefix.is_none() && parameter.name.local.to_string() == "pattern"
        })
        .collect::<Vec<_>>();
    // TODO Validate attribute values against multiple patterns that XSD
    // conjoins.
    let [parameter] = patterns.as_slice() else {
        return Ok(None);
    };

    resolve_pattern(&parameter.value)
        .map_err(|error| MacroError::XsdPattern(parameter.value.clone(), error))
}

// Translated patterns are anchored as XSD patterns match whole values, and
// compiled to verify their syntax.
fn resolve_pattern(pattern: &str) -> Result<Option<Literal>, XsdPatternError> {
    let Some(pattern) = translate_pattern(pattern)? else {
        return Ok(None);
    };
    let pattern = format!(r"\A(?:{pattern})\z");

    Regex::new(&pattern)?;

    Ok(Some(Literal::Pattern(pattern)))
}

// Translates an XSD pattern into a regular expression, fails on invalid
// patterns, or gives up on constructs it does not translate faithfully.
// Quantifiers and stray closing brackets pass through verbatim as the Nu Html
// Checker rejects schemas with those invalid in XSD.
//
// TODO Translate more constructs like name-character escapes, category
// escapes, and class subtractions.
// TODO Validate quantifier syntax against the XSD grammar.
fn translate_pattern(pattern: &str) -> Result<Option<String>, XsdPatternError> {
    let mut translated = String::new();
    let mut characters = pattern.chars().peekable();
    let mut class = false;
    let mut dash = false;
    let mut expansion = false;
    let mut groups = 0usize;

    while let Some(character) = characters.next() {
        let range = dash;
        let expanded = expansion;
        dash = false;
        expansion = false;

        match (character, class) {
            ('\\', _) => {
                let escaped = characters
                    .next()
                    .ok_or(XsdPatternError::TrailingBackslash)?;

                // Whitespace escapes expand into class members that must not
                // become range endpoints.
                expansion = class && escaped == 's';

                if expansion && range {
                    return Err(XsdPatternError::InvalidRange);
                }

                let Some(escape) = translate_escape(escaped, class)? else {
                    return Ok(None);
                };

                translated.push_str(&escape);
            }
            ('[', false) => {
                class = true;
                translated.push(character);
            }
            // Nested classes after dashes mean class subtraction.
            ('[', true) if range => return Ok(None),
            ('[', true) => return Err(XsdPatternError::UnescapedBracket),
            (']', true) => {
                class = false;
                translated.push(character);
            }
            // A dot matches any character but line breaks.
            ('.', false) => translated.push_str(r"[^\n\r]"),
            // Anchors are ordinary characters.
            ('^' | '$', false) => {
                translated.push('\\');
                translated.push(character);
            }
            ('(', false) => {
                groups += 1;
                translated.push(character);
            }
            (')', false) => {
                groups = groups
                    .checked_sub(1)
                    .ok_or(XsdPatternError::UnbalancedParentheses)?;
                translated.push(character);
            }
            ('-', true) if range => return Err(XsdPatternError::UnescapedDash),
            ('-', true) if expanded => {
                return if characters.peek() == Some(&'[') {
                    Ok(None)
                } else {
                    Err(XsdPatternError::InvalidRange)
                };
            }
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

    if class {
        Err(XsdPatternError::UnclosedClass)
    } else if groups > 0 {
        Err(XsdPatternError::UnbalancedParentheses)
    } else {
        Ok(Some(translated))
    }
}

fn translate_escape(character: char, class: bool) -> Result<Option<String>, XsdPatternError> {
    Ok(match (character, class) {
        (
            'n' | 'r' | 't' | 'd' | 'D' | '\\' | '|' | '.' | '?' | '*' | '+' | '(' | ')' | '{'
            | '}' | '-' | '[' | ']' | '^',
            _,
        ) => Some(format!("\\{character}")),
        ('s', false) => Some(r"[ \t\n\r]".into()),
        ('S', false) => Some(r"[^ \t\n\r]".into()),
        ('s', true) => Some(r" \t\n\r".into()),
        // Name-character, word-character, and category escapes have no
        // faithful translation, nor do negated whitespace escapes in classes.
        ('i' | 'I' | 'c' | 'C' | 'w' | 'W' | 'p' | 'P', _) | ('S', true) => None,
        _ => return Err(XsdPatternError::UnknownEscape(character)),
    })
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
                resolve_data(&datatype("xsd", "string"), &[parameter("pattern", "--.*")]).unwrap(),
                Some(Literal::Pattern(r"\A(?:--[^\n\r]*)\z".into()))
            );
        }

        #[test]
        fn resolve_pattern_with_length_parameter() {
            assert_eq!(
                resolve_data(
                    &datatype("xsd", "string"),
                    &[parameter("pattern", "a"), parameter("minLength", "1")]
                )
                .unwrap(),
                Some(Literal::Pattern(r"\A(?:a)\z".into()))
            );
        }

        #[test]
        fn resolve_pattern_with_literal_brace() {
            assert_eq!(
                resolve_data(&datatype("xsd", "string"), &[parameter("pattern", "a}")]).unwrap(),
                Some(Literal::Pattern(r"\A(?:a})\z".into()))
            );
        }

        #[test]
        fn resolve_no_string_without_parameter() {
            assert_eq!(resolve_data(&datatype("xsd", "string"), &[]).unwrap(), None);
        }

        #[test]
        fn resolve_no_token_datatype() {
            assert_eq!(
                resolve_data(&datatype("xsd", "token"), &[parameter("pattern", "a")]).unwrap(),
                None
            );
        }

        #[test]
        fn resolve_no_unknown_prefix() {
            assert_eq!(
                resolve_data(&datatype("w", "string"), &[parameter("pattern", "a")]).unwrap(),
                None
            );
        }

        #[test]
        fn resolve_no_built_in_datatype() {
            assert_eq!(
                resolve_data(&DatatypeName::String, &[parameter("pattern", "a")]).unwrap(),
                None
            );
        }

        #[test]
        fn resolve_no_multiple_patterns() {
            assert_eq!(
                resolve_data(
                    &datatype("xsd", "string"),
                    &[parameter("pattern", "a"), parameter("pattern", "b")]
                )
                .unwrap(),
                None
            );
        }

        #[test]
        fn resolve_no_untranslatable_pattern() {
            assert_eq!(
                resolve_data(
                    &datatype("xsd", "string"),
                    &[parameter("pattern", r"[\i-[:]]")]
                )
                .unwrap(),
                None
            );
        }

        #[test]
        fn fail_on_invalid_pattern() {
            assert!(matches!(
                resolve_data(&datatype("xsd", "string"), &[parameter("pattern", "a)")]),
                Err(MacroError::XsdPattern(pattern, XsdPatternError::UnbalancedParentheses))
                    if pattern == "a)"
            ));
        }

        #[test]
        fn fail_on_invalid_regex() {
            assert!(matches!(
                resolve_data(&datatype("xsd", "string"), &[parameter("pattern", "{2}")]),
                Err(MacroError::XsdPattern(pattern, XsdPatternError::Regex(_))) if pattern == "{2}"
            ));
        }
    }

    mod translate {
        use super::*;
        use pretty_assertions::assert_eq;

        #[test]
        fn translate_characters() {
            assert_eq!(translate_pattern("foo"), Ok(Some("foo".into())));
        }

        #[test]
        fn translate_empty_pattern() {
            assert_eq!(translate_pattern(""), Ok(Some("".into())));
        }

        #[test]
        fn translate_alternatives_with_quantifiers() {
            assert_eq!(
                translate_pattern("(very){0,2}(thin|thick)+x?"),
                Ok(Some("(very){0,2}(thin|thick)+x?".into()))
            );
        }

        #[test]
        fn translate_quantifier_ranges() {
            assert_eq!(
                translate_pattern("a{2}b{3,}c{4,5}"),
                Ok(Some("a{2}b{3,}c{4,5}".into()))
            );
        }

        #[test]
        fn keep_braces() {
            assert_eq!(translate_pattern("a}"), Ok(Some("a}".into())));
            assert_eq!(translate_pattern("x{2}{3}"), Ok(Some("x{2}{3}".into())));
            assert_eq!(translate_pattern("a{ 2}"), Ok(Some("a{ 2}".into())));
            assert_eq!(translate_pattern("a{2"), Ok(Some("a{2".into())));
        }

        #[test]
        fn keep_misplaced_quantifiers() {
            assert_eq!(translate_pattern("a**"), Ok(Some("a**".into())));
            assert_eq!(translate_pattern("a+?"), Ok(Some("a+?".into())));
            assert_eq!(translate_pattern("?a"), Ok(Some("?a".into())));
            assert_eq!(translate_pattern("a|+b"), Ok(Some("a|+b".into())));
            assert_eq!(translate_pattern("(?i)x"), Ok(Some("(?i)x".into())));
        }

        #[test]
        fn keep_closing_bracket() {
            assert_eq!(translate_pattern("a]"), Ok(Some("a]".into())));
        }

        #[test]
        fn translate_dot() {
            assert_eq!(translate_pattern("."), Ok(Some(r"[^\n\r]".into())));
        }

        #[test]
        fn translate_dot_in_class() {
            assert_eq!(translate_pattern("[.]"), Ok(Some("[.]".into())));
        }

        #[test]
        fn translate_anchor_characters() {
            assert_eq!(translate_pattern("a^b$"), Ok(Some(r"a\^b\$".into())));
        }

        #[test]
        fn translate_class() {
            assert_eq!(
                translate_pattern("[0-9a-fA-F]{6}"),
                Ok(Some("[0-9a-fA-F]{6}".into()))
            );
        }

        #[test]
        fn translate_negated_class() {
            assert_eq!(translate_pattern("[^ ]+"), Ok(Some("[^ ]+".into())));
        }

        #[test]
        fn translate_escaped_characters() {
            assert_eq!(
                translate_pattern(r"\.\{\}\(\)\[\]\-\+\*\?\|\\\^"),
                Ok(Some(r"\.\{\}\(\)\[\]\-\+\*\?\|\\\^".into()))
            );
        }

        #[test]
        fn translate_control_character_escapes() {
            assert_eq!(translate_pattern(r"\n\r\t"), Ok(Some(r"\n\r\t".into())));
        }

        #[test]
        fn translate_digit_escapes() {
            assert_eq!(translate_pattern(r"\d[\d]\D"), Ok(Some(r"\d[\d]\D".into())));
        }

        #[test]
        fn translate_whitespace_escapes() {
            assert_eq!(
                translate_pattern(r"\s*\S+"),
                Ok(Some(r"[ \t\n\r]*[^ \t\n\r]+".into()))
            );
        }

        #[test]
        fn translate_whitespace_escape_in_class() {
            assert_eq!(translate_pattern(r"[x\s]"), Ok(Some(r"[x \t\n\r]".into())));
        }

        #[test]
        fn translate_ampersand_and_tilde_in_class() {
            assert_eq!(translate_pattern("[&~]"), Ok(Some(r"[\&\~]".into())));
        }

        #[test]
        fn translate_no_name_character_escapes() {
            assert_eq!(translate_pattern(r"\i"), Ok(None));
            assert_eq!(translate_pattern(r"\c"), Ok(None));
            assert_eq!(translate_pattern(r"\I"), Ok(None));
            assert_eq!(translate_pattern(r"\C"), Ok(None));
        }

        #[test]
        fn translate_no_word_character_escapes() {
            assert_eq!(translate_pattern(r"\w"), Ok(None));
            assert_eq!(translate_pattern(r"\W"), Ok(None));
        }

        #[test]
        fn translate_no_category_escapes() {
            assert_eq!(translate_pattern(r"\p{Lu}"), Ok(None));
            assert_eq!(translate_pattern(r"\P{Lu}"), Ok(None));
        }

        #[test]
        fn translate_no_negated_whitespace_escape_in_class() {
            assert_eq!(translate_pattern(r"[\S]"), Ok(None));
        }

        #[test]
        fn translate_no_class_subtraction() {
            // cspell: ignore aeiou
            assert_eq!(translate_pattern("[a-z-[aeiou]]"), Ok(None));
            assert_eq!(translate_pattern("[a-[b]]"), Ok(None));
        }

        #[test]
        fn translate_no_class_subtraction_from_whitespace_escape() {
            assert_eq!(translate_pattern(r"[\s-[x]]"), Ok(None));
        }

        #[test]
        fn fail_on_unknown_escape() {
            assert_eq!(
                translate_pattern(r"\a"),
                Err(XsdPatternError::UnknownEscape('a'))
            );
            assert_eq!(
                translate_pattern(r"[\a]"),
                Err(XsdPatternError::UnknownEscape('a'))
            );
        }

        #[test]
        fn fail_on_trailing_backslash() {
            assert_eq!(
                translate_pattern("\\"),
                Err(XsdPatternError::TrailingBackslash)
            );
        }

        #[test]
        fn fail_on_unbalanced_parentheses() {
            assert_eq!(
                translate_pattern("(a"),
                Err(XsdPatternError::UnbalancedParentheses)
            );
            assert_eq!(
                translate_pattern("a)b"),
                Err(XsdPatternError::UnbalancedParentheses)
            );
            assert_eq!(
                translate_pattern("i)x|(y"),
                Err(XsdPatternError::UnbalancedParentheses)
            );
        }

        #[test]
        fn fail_on_unclosed_class() {
            assert_eq!(translate_pattern("[a"), Err(XsdPatternError::UnclosedClass));
        }

        #[test]
        fn fail_on_unescaped_bracket_in_class() {
            assert_eq!(
                translate_pattern("[[a]]"),
                Err(XsdPatternError::UnescapedBracket)
            );
            assert_eq!(
                translate_pattern("[a[b]]"),
                Err(XsdPatternError::UnescapedBracket)
            );
        }

        #[test]
        fn fail_on_class_difference() {
            assert_eq!(
                translate_pattern("[a-z--a]"),
                Err(XsdPatternError::UnescapedDash)
            );
            assert_eq!(
                translate_pattern("[a--b]"),
                Err(XsdPatternError::UnescapedDash)
            );
        }

        #[test]
        fn fail_on_whitespace_escape_starting_range() {
            assert_eq!(
                translate_pattern(r"[\s-x]"),
                Err(XsdPatternError::InvalidRange)
            );
        }

        #[test]
        fn fail_on_whitespace_escape_ending_range() {
            assert_eq!(
                translate_pattern(r"[a-\s]"),
                Err(XsdPatternError::InvalidRange)
            );
        }
    }
}
