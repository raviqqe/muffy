use regex::Regex;
use std::{
    collections::HashMap,
    sync::{LazyLock, PoisonError, RwLock},
};

pub enum Literal {
    CaseInsensitive(&'static str),
    Exact(&'static str),
    Pattern(&'static str),
    Token(&'static str),
}

impl Literal {
    pub fn matches(&self, value: &str) -> bool {
        match self {
            Self::CaseInsensitive(literal) => value.eq_ignore_ascii_case(literal),
            Self::Exact(literal) => value == *literal,
            Self::Pattern(literal) => matches_pattern(literal, value),
            // Whitespace around and within tokens is insignificant.
            Self::Token(literal) => value
                .split_ascii_whitespace()
                .eq(literal.split_ascii_whitespace()),
        }
    }
}

fn matches_pattern(pattern: &'static str, value: &str) -> bool {
    static REGEXES: LazyLock<RwLock<HashMap<&str, Option<Regex>>>> =
        LazyLock::new(Default::default);

    // Invalid patterns match any value conservatively although the macro
    // verifies patterns at compile time.
    let matches_value =
        |regex: &Option<Regex>| regex.as_ref().is_none_or(|regex| regex.is_match(value));

    if let Some(regex) = REGEXES
        .read()
        .unwrap_or_else(PoisonError::into_inner)
        .get(pattern)
    {
        matches_value(regex)
    } else {
        matches_value(
            REGEXES
                .write()
                .unwrap_or_else(PoisonError::into_inner)
                .entry(pattern)
                .or_insert_with(|| Regex::new(pattern).ok()),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn match_exact_literal() {
        const LITERAL: Literal = Literal::Exact("foo");

        assert!(LITERAL.matches("foo"));
        assert!(!LITERAL.matches("FOO"));
        assert!(!LITERAL.matches(" foo "));
        assert!(!LITERAL.matches("bar"));
    }

    #[test]
    fn match_empty_exact_literal() {
        const LITERAL: Literal = Literal::Exact("");

        assert!(LITERAL.matches(""));
        assert!(!LITERAL.matches("foo"));
    }

    #[test]
    fn match_case_insensitive_literal() {
        const LITERAL: Literal = Literal::CaseInsensitive("foo");

        assert!(LITERAL.matches("foo"));
        assert!(LITERAL.matches("FOO"));
        assert!(LITERAL.matches("Foo"));
        assert!(!LITERAL.matches(" foo"));
        assert!(!LITERAL.matches("bar"));
    }

    #[test]
    fn match_pattern_literal() {
        const LITERAL: Literal = Literal::Pattern(r"\A(?:--[^\n\r]*)\z");

        assert!(LITERAL.matches("--"));
        assert!(LITERAL.matches("--foo"));
        assert!(!LITERAL.matches(""));
        assert!(!LITERAL.matches("foo"));
        assert!(!LITERAL.matches(" --foo"));
    }

    #[test]
    fn match_pattern_literal_repeatedly() {
        const LITERAL: Literal = Literal::Pattern(r"\A(?:a+)\z");

        assert!(LITERAL.matches("aa"));
        assert!(LITERAL.matches("a"));
        assert!(!LITERAL.matches("b"));
    }

    #[test]
    fn match_any_value_against_invalid_pattern_literal() {
        const LITERAL: Literal = Literal::Pattern("(");

        assert!(LITERAL.matches(""));
        assert!(LITERAL.matches("foo"));
    }

    #[test]
    fn match_token_literal() {
        const LITERAL: Literal = Literal::Token("foo");

        assert!(LITERAL.matches("foo"));
        assert!(LITERAL.matches(" foo "));
        assert!(LITERAL.matches("\tfoo\n"));
        assert!(!LITERAL.matches("FOO"));
        assert!(!LITERAL.matches("bar"));
    }

    #[test]
    fn match_multi_token_literal() {
        const LITERAL: Literal = Literal::Token("foo bar");

        assert!(LITERAL.matches("foo bar"));
        assert!(LITERAL.matches(" foo  bar "));
        assert!(!LITERAL.matches("foo"));
        assert!(!LITERAL.matches("foobar"));
    }
}
