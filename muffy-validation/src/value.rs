pub enum Value {
    Any,
    LiteralSet(&'static [Literal]),
}

impl Value {
    pub fn matches(&self, value: &str) -> bool {
        match self {
            Self::Any => true,
            Self::LiteralSet(literals) => literals.iter().any(|literal| literal.matches(value)),
        }
    }
}

pub enum Literal {
    CaseInsensitive(&'static str),
    Exact(&'static str),
    Token(&'static str),
}

impl Literal {
    fn matches(&self, value: &str) -> bool {
        match self {
            Self::CaseInsensitive(literal) => value.eq_ignore_ascii_case(literal),
            Self::Exact(literal) => value == *literal,
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
    fn match_any_value() {
        assert!(Value::Any.matches(""));
        assert!(Value::Any.matches("foo"));
    }

    #[test]
    fn match_no_literal() {
        assert!(!Value::LiteralSet(&[]).matches(""));
        assert!(!Value::LiteralSet(&[]).matches("foo"));
    }

    #[test]
    fn match_alternative_literals() {
        const VALUE: Value = Value::LiteralSet(&[Literal::Exact("foo"), Literal::Exact("bar")]);

        assert!(VALUE.matches("foo"));
        assert!(VALUE.matches("bar"));
        assert!(!VALUE.matches("baz"));
    }

    #[test]
    fn match_exact_literal() {
        const VALUE: Value = Value::LiteralSet(&[Literal::Exact("foo")]);

        assert!(VALUE.matches("foo"));
        assert!(!VALUE.matches("FOO"));
        assert!(!VALUE.matches(" foo "));
    }

    #[test]
    fn match_empty_exact_literal() {
        const VALUE: Value = Value::LiteralSet(&[Literal::Exact("")]);

        assert!(VALUE.matches(""));
        assert!(!VALUE.matches("foo"));
    }

    #[test]
    fn match_case_insensitive_literal() {
        const VALUE: Value = Value::LiteralSet(&[Literal::CaseInsensitive("foo")]);

        assert!(VALUE.matches("foo"));
        assert!(VALUE.matches("FOO"));
        assert!(VALUE.matches("Foo"));
        assert!(!VALUE.matches(" foo"));
        assert!(!VALUE.matches("bar"));
    }

    #[test]
    fn match_token_literal() {
        const VALUE: Value = Value::LiteralSet(&[Literal::Token("foo")]);

        assert!(VALUE.matches("foo"));
        assert!(VALUE.matches(" foo "));
        assert!(VALUE.matches("\tfoo\n"));
        assert!(!VALUE.matches("FOO"));
        assert!(!VALUE.matches("bar"));
    }

    #[test]
    fn match_multi_token_literal() {
        const VALUE: Value = Value::LiteralSet(&[Literal::Token("foo bar")]);

        assert!(VALUE.matches("foo bar"));
        assert!(VALUE.matches(" foo  bar "));
        assert!(!VALUE.matches("foo"));
        assert!(!VALUE.matches("foobar"));
    }
}
