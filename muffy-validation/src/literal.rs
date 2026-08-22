use regex::Regex;

// Compiles a pattern literal verified already.
macro_rules! pattern {
    ($pattern:literal) => {
        $crate::literal::Literal::Pattern(|| {
            static REGEX: ::std::sync::LazyLock<::regex::Regex> =
                ::std::sync::LazyLock::new(|| ::regex::Regex::new($pattern).unwrap());

            &REGEX
        })
    };
}

pub(crate) use pattern;

pub enum Literal {
    CaseInsensitive(&'static str),
    Exact(&'static str),
    Pattern(fn() -> &'static Regex),
    Token(&'static str),
}

impl Literal {
    pub fn matches(&self, value: &str) -> bool {
        match self {
            Self::CaseInsensitive(literal) => value.eq_ignore_ascii_case(literal),
            Self::Exact(literal) => value == *literal,
            Self::Pattern(pattern) => pattern().is_match(value),
            // Whitespace around and within tokens is insignificant.
            Self::Token(literal) => value
                .split_ascii_whitespace()
                .eq(literal.split_ascii_whitespace()),
        }
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
        const LITERAL: Literal = pattern!(r"\A(?:--[^\n\r]*)\z");

        assert!(LITERAL.matches("--"));
        assert!(LITERAL.matches("--foo"));
        assert!(!LITERAL.matches(""));
        assert!(!LITERAL.matches("foo"));
        assert!(!LITERAL.matches(" --foo"));
    }

    #[test]
    fn match_pattern_literal_repeatedly() {
        const LITERAL: Literal = pattern!(r"\A(?:a+)\z");

        assert!(LITERAL.matches("aa"));
        assert!(LITERAL.matches("a"));
        assert!(!LITERAL.matches("b"));
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
