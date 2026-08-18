use crate::literal::Literal;

pub enum Value {
    Any,
    LiteralSet(&'static [Literal]),
    TokenList(&'static [Literal]),
}

impl Value {
    pub fn matches(&self, value: &str) -> bool {
        match self {
            Self::Any => true,
            Self::LiteralSet(literals) => matches_literal(literals, value),
            // Browsers process only the first recognized token, so a value is
            // valid as long as it names something known.
            Self::TokenList(literals) => value
                .split_ascii_whitespace()
                .any(|token| matches_literal(literals, token)),
        }
    }
}

fn matches_literal(literals: &[Literal], value: &str) -> bool {
    literals.iter().any(|literal| literal.matches(value))
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

    mod token_list {
        use super::*;

        const VALUE: Value = Value::TokenList(&[Literal::Exact("foo"), Literal::Exact("bar")]);

        #[test]
        fn match_single_token() {
            assert!(VALUE.matches("foo"));
            assert!(VALUE.matches("bar"));
            assert!(!VALUE.matches("baz"));
        }

        #[test]
        fn match_surrounded_token() {
            assert!(VALUE.matches(" foo "));
            assert!(VALUE.matches("\tfoo\n"));
        }

        #[test]
        fn match_leading_token() {
            assert!(VALUE.matches("foo baz"));
        }

        #[test]
        fn match_trailing_token() {
            assert!(VALUE.matches("baz foo"));
        }

        #[test]
        fn match_no_token() {
            assert!(!VALUE.matches("baz qux"));
        }

        #[test]
        fn match_no_empty_value() {
            assert!(!VALUE.matches(""));
            assert!(!VALUE.matches("  "));
        }
    }
}
