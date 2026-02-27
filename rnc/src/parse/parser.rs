use crate::{
    Identifier,
    ast::{
        AnnotationAttribute, AnnotationElement, Combine, DatatypesDeclaration, Declaration,
        Definition, Grammar, GrammarContent, Include, Inherit, Name, NameClass,
        NamespaceDeclaration, Parameter, Pattern, Schema, SchemaBody,
    },
};
use nom::{
    IResult, Parser,
    branch::alt,
    bytes::complete::{escaped_transform, is_not, tag, take, take_till},
    character::complete::{alpha1, char, multispace1, satisfy},
    combinator::{all_consuming, map, not, opt, peek, recognize, value, verify},
    error::Error,
    multi::{many0, separated_list0, separated_list1},
    sequence::{delimited, preceded, terminated},
};

type ParserError<'input> = Error<&'input str>;

type ParserResult<'input, Output> = IResult<&'input str, Output, ParserError<'input>>;

pub fn schema(input: &str) -> ParserResult<'_, Schema> {
    map(
        blanked((many0(declaration), schema_body)),
        |(declarations, body)| Schema { declarations, body },
    )
    .parse(input)
}

fn schema_body(input: &str) -> ParserResult<'_, SchemaBody> {
    alt((
        // TODO Remove the all consuming combinator.
        map(all_consuming(grammar), SchemaBody::Grammar),
        map(pattern, SchemaBody::Pattern),
    ))
    .parse(input)
}

fn declaration(input: &str) -> ParserResult<'_, Declaration> {
    blanked(alt((
        map(namespace_declaration, Declaration::Namespace),
        map(default_namespace_declaration, Declaration::DefaultNamespace),
        map(datatypes_declaration, Declaration::Datatypes),
    )))
    .parse(input)
}

fn namespace_declaration(input: &str) -> ParserResult<'_, NamespaceDeclaration> {
    map(
        (keyword("namespace"), identifier, symbol("="), literal),
        |(_, prefix, _, uri)| NamespaceDeclaration { prefix, uri },
    )
    .parse(input)
}

fn default_namespace_declaration(input: &str) -> ParserResult<'_, String> {
    map(
        (
            keyword("default"),
            keyword("namespace"),
            opt(identifier),
            symbol("="),
            literal,
        ),
        |(_, _, _, _, uri)| uri,
    )
    .parse(input)
}

fn datatypes_declaration(input: &str) -> ParserResult<'_, DatatypesDeclaration> {
    map(
        (keyword("datatypes"), opt(identifier), symbol("="), literal),
        |(_, prefix, _, uri)| DatatypesDeclaration { prefix, uri },
    )
    .parse(input)
}

fn grammar(input: &str) -> ParserResult<'_, Grammar> {
    map(many0(grammar_content), |items| Grammar { contents: items }).parse(input)
}

fn grammar_content(input: &str) -> ParserResult<'_, GrammarContent> {
    annotated(alt((
        map(annotation_element, GrammarContent::Annotation),
        start,
        definition,
        div,
        include,
    )))
    .parse(input)
}

fn start(input: &str) -> ParserResult<'_, GrammarContent> {
    map(
        (keyword("start"), assignment_operator, pattern),
        |(_, combine, pattern)| GrammarContent::Start { combine, pattern },
    )
    .parse(input)
}

fn definition(input: &str) -> ParserResult<'_, GrammarContent> {
    map(
        (identifier, assignment_operator, pattern),
        |(name, combine, pattern)| {
            GrammarContent::Definition(Definition {
                name,
                combine,
                pattern,
            })
        },
    )
    .parse(input)
}

fn div(input: &str) -> ParserResult<'_, GrammarContent> {
    map(
        preceded(keyword("div"), braced(grammar)),
        GrammarContent::Div,
    )
    .parse(input)
}

fn include(input: &str) -> ParserResult<'_, GrammarContent> {
    map(
        (
            keyword("include"),
            literal,
            opt(inherit),
            opt(braced(grammar)),
        ),
        |(_, uri, inherit, grammar)| {
            GrammarContent::Include(Include {
                uri,
                inherit,
                grammar,
            })
        },
    )
    .parse(input)
}

fn inherit(input: &str) -> ParserResult<'_, Inherit> {
    map(
        preceded((keyword("inherit"), symbol("=")), identifier),
        Inherit::Prefix,
    )
    .parse(input)
}

fn assignment_operator(input: &str) -> ParserResult<'_, Option<Combine>> {
    alt((
        value(Some(Combine::Choice), symbol("|=")),
        value(Some(Combine::Interleave), symbol("&=")),
        value(None, symbol("=")),
    ))
    .parse(input)
}

fn pattern(input: &str) -> ParserResult<'_, Pattern> {
    map(
        separated_list1(symbol("|"), interleave_pattern),
        |patterns| fold_patterns(patterns, Pattern::Choice),
    )
    .parse(input)
}

fn interleave_pattern(input: &str) -> ParserResult<'_, Pattern> {
    map(separated_list1(symbol("&"), group_pattern), |patterns| {
        fold_patterns(patterns, Pattern::Interleave)
    })
    .parse(input)
}

fn group_pattern(input: &str) -> ParserResult<'_, Pattern> {
    map(
        separated_list1(symbol(","), quantified_pattern),
        |patterns| fold_patterns(patterns, Pattern::Group),
    )
    .parse(input)
}

fn quantified_pattern(input: &str) -> ParserResult<'_, Pattern> {
    map(
        (
            primary_pattern,
            many0(follow_annotation),
            opt(alt((
                value("?", symbol("?")),
                value("*", symbol("*")),
                value("+", symbol("+")),
            ))),
        ),
        |(pattern, _, quantifier)| match quantifier {
            Some("?") => Pattern::Optional(pattern.into()),
            Some("*") => Pattern::Many0(pattern.into()),
            Some("+") => Pattern::Many1(pattern.into()),
            // TODO Prevent an invalid quantifier.
            Some(_) | None => pattern,
        },
    )
    .parse(input)
}

fn primary_pattern(input: &str) -> ParserResult<'_, Pattern> {
    annotated(alt((
        element_pattern,
        attribute_pattern,
        list_pattern,
        grammar_pattern,
        external_pattern,
        text_pattern,
        empty_pattern,
        not_allowed_pattern,
        value_pattern,
        data_pattern,
        name_pattern,
        parenthesized(pattern),
    )))
    .parse(input)
}

fn element_pattern(input: &str) -> ParserResult<'_, Pattern> {
    map(
        (keyword("element"), name_class, braced(pattern)),
        |(_, name_class, pattern)| Pattern::Element {
            name_class,
            pattern: pattern.into(),
        },
    )
    .parse(input)
}

fn attribute_pattern(input: &str) -> ParserResult<'_, Pattern> {
    map(
        (keyword("attribute"), name_class, braced(pattern)),
        |(_, name_class, pattern)| Pattern::Attribute {
            name_class,
            pattern: pattern.into(),
        },
    )
    .parse(input)
}

fn list_pattern(input: &str) -> ParserResult<'_, Pattern> {
    map(preceded(keyword("list"), braced(pattern)), |pattern| {
        Pattern::List(pattern.into())
    })
    .parse(input)
}

fn grammar_pattern(input: &str) -> ParserResult<'_, Pattern> {
    map(
        preceded(keyword("grammar"), braced(grammar)),
        Pattern::Grammar,
    )
    .parse(input)
}

fn external_pattern(input: &str) -> ParserResult<'_, Pattern> {
    map(preceded(keyword("external"), literal), Pattern::External).parse(input)
}

fn text_pattern(input: &str) -> ParserResult<'_, Pattern> {
    value(Pattern::Text, keyword("text")).parse(input)
}

fn empty_pattern(input: &str) -> ParserResult<'_, Pattern> {
    value(Pattern::Empty, keyword("empty")).parse(input)
}

fn not_allowed_pattern(input: &str) -> ParserResult<'_, Pattern> {
    value(Pattern::NotAllowed, keyword("notAllowed")).parse(input)
}

fn name_pattern(input: &str) -> ParserResult<'_, Pattern> {
    map(name, Pattern::Name).parse(input)
}

fn data_pattern(input: &str) -> ParserResult<'_, Pattern> {
    map(
        verify(
            (name, opt(parameters), opt(preceded(symbol("-"), pattern))),
            |(_, parameters, except_pattern)| parameters.is_some() || except_pattern.is_some(),
        ),
        |(name, parameters, except_pattern)| Pattern::Data {
            name,
            parameters: parameters.unwrap_or_default(),
            except: except_pattern.map(Box::new),
        },
    )
    .parse(input)
}

fn value_pattern(input: &str) -> ParserResult<'_, Pattern> {
    map((opt(name), literal), |(name, value)| Pattern::Value {
        name,
        value,
    })
    .parse(input)
}

fn name_class(input: &str) -> ParserResult<'_, NameClass> {
    map(
        separated_list1(symbol("|"), name_class_choice),
        |mut classes| {
            if classes.len() == 1
                && let Some(class) = classes.pop()
            {
                class
            } else {
                NameClass::Choice(classes)
            }
        },
    )
    .parse(input)
}

fn name_class_choice(input: &str) -> ParserResult<'_, NameClass> {
    map(
        (
            primary_name_class,
            opt(preceded(symbol("-"), primary_name_class)),
        ),
        |(base, except)| match except {
            Some(except) => NameClass::Except {
                base: Box::new(base),
                except: Box::new(except),
            },
            None => base,
        },
    )
    .parse(input)
}

fn primary_name_class(input: &str) -> ParserResult<'_, NameClass> {
    blanked(alt((
        value(NameClass::AnyName, tag("*")),
        map((raw_identifier, char(':'), char('*')), |(prefix, _, _)| {
            NameClass::NamespaceName(Some(prefix))
        }),
        map(name, NameClass::Name),
        parenthesized(name_class),
    )))
    .parse(input)
}

fn parameters(input: &str) -> ParserResult<'_, Vec<Parameter>> {
    braced(separated_list0(opt(symbol(",")), parameter)).parse(input)
}

fn parameter(input: &str) -> ParserResult<'_, Parameter> {
    map((name, symbol("="), literal), |(name, _, value)| Parameter {
        name,
        value,
    })
    .parse(input)
}

fn annotation_element(input: &str) -> ParserResult<'_, AnnotationElement> {
    map(
        (
            name,
            bracketed((
                many0(annotation_attribute),
                many0(alt((value((), annotation_element), value((), literal)))),
            )),
        ),
        |(name, (attributes, _))| AnnotationElement { name, attributes },
    )
    .parse(input)
}

fn annotation(input: &str) -> ParserResult<'_, (Vec<AnnotationAttribute>, Vec<AnnotationElement>)> {
    bracketed((many0(annotation_attribute), many0(annotation_element))).parse(input)
}

fn annotation_attribute(input: &str) -> ParserResult<'_, AnnotationAttribute> {
    map((name, symbol("="), literal), |(name, _, value)| {
        AnnotationAttribute { name, value }
    })
    .parse(input)
}

fn annotated<'a, T>(
    parser: impl Parser<&'a str, Output = T, Error = ParserError<'a>>,
) -> impl Parser<&'a str, Output = T, Error = ParserError<'a>> {
    preceded(many0(annotation), parser)
}

fn follow_annotation(input: &str) -> ParserResult<'_, ()> {
    value((), (symbol(">>"), annotation_element)).parse(input)
}

fn name(input: &str) -> ParserResult<'_, Name> {
    map(
        blanked((opt(terminated(raw_identifier, char(':'))), raw_identifier)),
        |(prefix, local)| Name { prefix, local },
    )
    .parse(input)
}

fn identifier(input: &str) -> ParserResult<'_, Identifier> {
    blanked(raw_identifier).parse(input)
}

fn raw_identifier(input: &str) -> ParserResult<'_, Identifier> {
    map(
        preceded(
            opt(char('\\')),
            separated_list1(
                char('.'),
                recognize((alpha1, many0(satisfy(is_identifier_char)))),
            ),
        ),
        |mut parts: Vec<&str>| Identifier {
            component: parts.remove(0).to_owned(),
            sub_components: parts.into_iter().map(ToOwned::to_owned).collect(),
        },
    )
    .parse(input)
}

fn literal(input: &str) -> ParserResult<'_, String> {
    map(separated_list1(symbol("~"), literal_segment), |segments| {
        segments.join("")
    })
    .parse(input)
}

fn literal_segment(input: &str) -> ParserResult<'_, String> {
    blanked(alt((quoted('"', "\\\""), quoted('\'', "\\'")))).parse(input)
}

fn quoted<'a>(
    delimiter: char,
    not: &'static str,
) -> impl Parser<&'a str, Output = String, Error = ParserError<'a>> {
    map(
        delimited(
            char(delimiter),
            opt(escaped_transform(is_not(not), '\\', string_escape)),
            char(delimiter),
        ),
        |string| string.unwrap_or_default(),
    )
}

fn keyword<'a>(
    keyword: &'static str,
) -> impl Parser<&'a str, Output = &'a str, Error = ParserError<'a>> {
    blanked(terminated(
        tag(keyword),
        not(peek(satisfy(is_identifier_char))),
    ))
}

fn symbol<'a>(
    symbol: &'static str,
) -> impl Parser<&'a str, Output = &'a str, Error = ParserError<'a>> {
    blanked(tag(symbol))
}

fn parenthesized<'a, T>(
    parser: impl Parser<&'a str, Output = T, Error = ParserError<'a>>,
) -> impl Parser<&'a str, Output = T, Error = ParserError<'a>> {
    delimited(symbol("("), parser, symbol(")"))
}

fn braced<'a, T>(
    parser: impl Parser<&'a str, Output = T, Error = ParserError<'a>>,
) -> impl Parser<&'a str, Output = T, Error = ParserError<'a>> {
    delimited(symbol("{"), parser, symbol("}"))
}

fn bracketed<'a, T>(
    parser: impl Parser<&'a str, Output = T, Error = ParserError<'a>>,
) -> impl Parser<&'a str, Output = T, Error = ParserError<'a>> {
    delimited(symbol("["), parser, symbol("]"))
}

fn blanked<'a, T>(
    parser: impl Parser<&'a str, Output = T, Error = ParserError<'a>>,
) -> impl Parser<&'a str, Output = T, Error = ParserError<'a>> {
    delimited(blank, parser, blank)
}

fn blank(input: &str) -> ParserResult<'_, ()> {
    value((), many0(alt((value((), multispace1), comment)))).parse(input)
}

fn comment(input: &str) -> ParserResult<'_, ()> {
    value(
        (),
        (
            preceded(tag("#"), take_till(|character| character == '\n')),
            opt(char('\n')),
        ),
    )
    .parse(input)
}

fn string_escape(input: &str) -> ParserResult<'_, &str> {
    alt((
        value("\\", tag("\\")),
        value("\"", tag("\"")),
        value("'", tag("'")),
        value("\n", tag("n")),
        value("\r", tag("r")),
        value("\t", tag("t")),
        take(1usize),
    ))
    .parse(input)
}

const fn is_identifier_char(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == '_' || character == '-'
}

fn fold_patterns(mut patterns: Vec<Pattern>, constructor: fn(Vec<Pattern>) -> Pattern) -> Pattern {
    if patterns.len() == 1
        && let Some(pattern) = patterns.pop()
    {
        pattern
    } else {
        constructor(patterns)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use indoc::indoc;

    mod identifier {
        use super::*;
        use pretty_assertions::assert_eq;

        #[test]
        fn parse_html() {
            assert_eq!(
                identifier("html"),
                Ok((
                    "",
                    Identifier {
                        component: "html".into(),
                        sub_components: vec![],
                    }
                ))
            );
        }

        #[test]
        fn parse_svg() {
            assert_eq!(
                identifier("svg"),
                Ok((
                    "",
                    Identifier {
                        component: "svg".into(),
                        sub_components: vec![],
                    }
                ))
            );
        }

        #[test]
        fn parse_common_attributes() {
            assert_eq!(
                identifier("common.attributes"),
                Ok((
                    "",
                    Identifier {
                        component: "common".into(),
                        sub_components: vec!["attributes".into()],
                    }
                ))
            );
        }

        #[test]
        fn parse_escaped_keyword() {
            assert_eq!(
                identifier("\\element"),
                Ok((
                    "",
                    Identifier {
                        component: "element".into(),
                        sub_components: vec![],
                    }
                ))
            );
        }

        #[test]
        fn fail_on_invalid_character() {
            assert!(identifier("!invalid").is_err());
        }
    }

    mod literal {
        use super::*;
        use pretty_assertions::assert_eq;

        #[test]
        fn parse_double_quoted_string() {
            assert_eq!(literal("\"http://www.w3.org/1999/xhtml\""), Ok(("", "http://www.w3.org/1999/xhtml".into())));
        }

        #[test]
        fn parse_single_quoted_string() {
            assert_eq!(literal("'http://www.w3.org/2000/svg'"), Ok(("", "http://www.w3.org/2000/svg".into())));
        }

        #[test]
        fn parse_concatenated_literals() {
            assert_eq!(literal("\"foo\" ~ \"bar\""), Ok(("", "foobar".into())));
        }

        #[test]
        fn parse_escaped_characters() {
            assert_eq!(literal("\"\\\"\\\\\\n\\r\\t\""), Ok(("", "\"\\\n\r\t".into())));
        }
    }

    mod name {
        use super::*;
        use pretty_assertions::assert_eq;

        #[test]
        fn parse_unprefixed_name() {
            assert_eq!(
                name("div"),
                Ok((
                    "",
                    Name {
                        prefix: None,
                        local: Identifier {
                            component: "div".into(),
                            sub_components: vec![],
                        },
                    }
                ))
            );
        }

        #[test]
        fn parse_prefixed_name() {
            assert_eq!(
                name("html:div"),
                Ok((
                    "",
                    Name {
                        prefix: Some(Identifier {
                            component: "html".into(),
                            sub_components: vec![],
                        }),
                        local: Identifier {
                            component: "div".into(),
                            sub_components: vec![],
                        },
                    }
                ))
            );
        }

        #[test]
        fn parse_prefixed_name_with_dots() {
            assert_eq!(
                name("xsd:integer"),
                Ok((
                    "",
                    Name {
                        prefix: Some(Identifier {
                            component: "xsd".into(),
                            sub_components: vec![],
                        }),
                        local: Identifier {
                            component: "integer".into(),
                            sub_components: vec![],
                        },
                    }
                ))
            );
        }
    }

    mod blank {
        use super::*;
        use pretty_assertions::assert_eq;

        #[test]
        fn parse_whitespace() {
            assert_eq!(blank("  \n\t  "), Ok(("", ())));
        }

        #[test]
        fn parse_comments() {
            assert_eq!(blank("# comment\n# another"), Ok(("", ())));
        }

        #[test]
        fn parse_mixed_whitespace_and_comments() {
            assert_eq!(blank("  # comment\n  "), Ok(("", ())));
        }
    }

    mod name_class {
        use super::*;
        use pretty_assertions::assert_eq;

        #[test]
        fn parses_any_name() {
            assert_eq!(name_class("*"), Ok(("", NameClass::AnyName)));
        }

        #[test]
        fn parses_ns_name() {
            assert_eq!(
                name_class("html:*"),
                Ok((
                    "",
                    NameClass::NamespaceName(Some(Identifier {
                        component: "html".into(),
                        sub_components: vec![],
                    }))
                ))
            );
        }

        #[test]
        fn parses_name() {
            assert_eq!(
                name_class("svg:rect"),
                Ok((
                    "",
                    NameClass::Name(Name {
                        prefix: Some(Identifier {
                            component: "svg".into(),
                            sub_components: vec![],
                        }),
                        local: Identifier {
                            component: "rect".into(),
                            sub_components: vec![],
                        },
                    })
                ))
            );
        }

        #[test]
        fn parses_choice() {
            assert_eq!(
                name_class("html:div | html:span"),
                Ok((
                    "",
                    NameClass::Choice(vec![
                        NameClass::Name(Name {
                            prefix: Some(Identifier {
                                component: "html".into(),
                                sub_components: vec![],
                            }),
                            local: Identifier {
                                component: "div".into(),
                                sub_components: vec![],
                            },
                        }),
                        NameClass::Name(Name {
                            prefix: Some(Identifier {
                                component: "html".into(),
                                sub_components: vec![],
                            }),
                            local: Identifier {
                                component: "span".into(),
                                sub_components: vec![],
                            },
                        }),
                    ])
                ))
            );
        }

        #[test]
        fn parses_except() {
            assert_eq!(
                name_class("html:* - html:script"),
                Ok((
                    "",
                    NameClass::Except {
                        base: Box::new(NameClass::NamespaceName(Some(Identifier {
                            component: "html".into(),
                            sub_components: vec![],
                        }))),
                        except: Box::new(NameClass::Name(Name {
                            prefix: Some(Identifier {
                                component: "html".into(),
                                sub_components: vec![],
                            }),
                            local: Identifier {
                                component: "script".into(),
                                sub_components: vec![],
                            },
                        })),
                    }
                ))
            );
        }
    }

    mod pattern {
        use super::*;
        use pretty_assertions::assert_eq;

        #[test]
        fn parses_element() {
            assert_eq!(
                pattern("element html { empty }"),
                Ok((
                    "",
                    Pattern::Element {
                        name_class: NameClass::Name(Name {
                            prefix: None,
                            local: Identifier {
                                component: "html".into(),
                                sub_components: vec![],
                            },
                        }),
                        pattern: Box::new(Pattern::Empty),
                    }
                ))
            );
        }

        #[test]
        fn parses_attribute() {
            assert_eq!(
                pattern("attribute id { text }"),
                Ok((
                    "",
                    Pattern::Attribute {
                        name_class: NameClass::Name(Name {
                            prefix: None,
                            local: Identifier {
                                component: "id".into(),
                                sub_components: vec![],
                            },
                        }),
                        pattern: Box::new(Pattern::Text),
                    }
                ))
            );
        }

        #[test]
        fn parses_choice() {
            assert_eq!(
                pattern("text | empty"),
                Ok((
                    "",
                    Pattern::Choice(vec![Pattern::Text, Pattern::Empty])
                ))
            );
        }

        #[test]
        fn parses_interleave() {
            assert_eq!(
                pattern("text & empty"),
                Ok((
                    "",
                    Pattern::Interleave(vec![Pattern::Text, Pattern::Empty])
                ))
            );
        }

        #[test]
        fn parses_group() {
            assert_eq!(
                pattern("text , empty"),
                Ok((
                    "",
                    Pattern::Group(vec![Pattern::Text, Pattern::Empty])
                ))
            );
        }

        #[test]
        fn parses_quantifiers() {
            assert_eq!(pattern("text?"), Ok(("", Pattern::Optional(Box::new(Pattern::Text)))));
            assert_eq!(pattern("text*"), Ok(("", Pattern::Many0(Box::new(Pattern::Text)))));
            assert_eq!(pattern("text+"), Ok(("", Pattern::Many1(Box::new(Pattern::Text)))));
        }

        #[test]
        fn parses_data() {
            assert_eq!(
                pattern("xsd:integer { minInclusive = \"1\" }"),
                Ok((
                    "",
                    Pattern::Data {
                        name: Name {
                            prefix: Some(Identifier {
                                component: "xsd".into(),
                                sub_components: vec![],
                            }),
                            local: Identifier {
                                component: "integer".into(),
                                sub_components: vec![],
                            },
                        },
                        parameters: vec![Parameter {
                            name: Name {
                                prefix: None,
                                local: Identifier {
                                    component: "minInclusive".into(),
                                    sub_components: vec![],
                                },
                            },
                            value: "1".into(),
                        }],
                        except: None,
                    }
                ))
            );
        }

        #[test]
        fn parses_name_pattern() {
            assert_eq!(
                pattern("xsd:integer"),
                Ok((
                    "",
                    Pattern::Name(Name {
                        prefix: Some(Identifier {
                            component: "xsd".into(),
                            sub_components: vec![],
                        }),
                        local: Identifier {
                            component: "integer".into(),
                            sub_components: vec![],
                        },
                    })
                ))
            );
        }

        #[test]
        fn parses_value() {
            assert_eq!(
                pattern("string \"auto\""),
                Ok((
                    "",
                    Pattern::Value {
                        name: Some(Name {
                            prefix: None,
                            local: Identifier {
                                component: "string".into(),
                                sub_components: vec![],
                            },
                        }),
                        value: "auto".into(),
                    }
                ))
            );
        }

        #[test]
        fn respects_precedence() {
            // , > & > |
            assert_eq!(
                pattern("a , b & c | d"),
                Ok((
                    "",
                    Pattern::Choice(vec![
                        Pattern::Interleave(vec![
                            Pattern::Group(vec![
                                Pattern::Name(Name { prefix: None, local: Identifier { component: "a".into(), sub_components: vec![] } }),
                                Pattern::Name(Name { prefix: None, local: Identifier { component: "b".into(), sub_components: vec![] } }),
                            ]),
                            Pattern::Name(Name { prefix: None, local: Identifier { component: "c".into(), sub_components: vec![] } }),
                        ]),
                        Pattern::Name(Name { prefix: None, local: Identifier { component: "d".into(), sub_components: vec![] } }),
                    ])
                ))
            );
        }
    }

    mod declaration {
        use super::*;
        use pretty_assertions::assert_eq;

        #[test]
        fn parses_namespace_declaration() {
            assert_eq!(
                declaration("namespace html = \"http://www.w3.org/1999/xhtml\""),
                Ok((
                    "",
                    Declaration::Namespace(NamespaceDeclaration {
                        prefix: Identifier {
                            component: "html".into(),
                            sub_components: vec![],
                        },
                        uri: "http://www.w3.org/1999/xhtml".into(),
                    })
                ))
            );
        }

        #[test]
        fn parses_default_namespace_declaration() {
            assert_eq!(
                declaration("default namespace = \"http://www.w3.org/1999/xhtml\""),
                Ok((
                    "",
                    Declaration::DefaultNamespace("http://www.w3.org/1999/xhtml".into())
                ))
            );
        }

        #[test]
        fn parses_datatypes_declaration() {
            assert_eq!(
                declaration("datatypes xsd = \"http://www.w3.org/2001/XMLSchema-datatypes\""),
                Ok((
                    "",
                    Declaration::Datatypes(DatatypesDeclaration {
                        prefix: Some(Identifier {
                            component: "xsd".into(),
                            sub_components: vec![],
                        }),
                        uri: "http://www.w3.org/2001/XMLSchema-datatypes".into(),
                    })
                ))
            );
        }
    }

    mod grammar {
        use super::*;
        use pretty_assertions::assert_eq;

        #[test]
        fn parses_start() {
            assert_eq!(
                grammar("start = element html { empty }"),
                Ok((
                    "",
                    Grammar {
                        contents: vec![GrammarContent::Start {
                            combine: None,
                            pattern: Pattern::Element {
                                name_class: NameClass::Name(Name {
                                    prefix: None,
                                    local: Identifier {
                                        component: "html".into(),
                                        sub_components: vec![],
                                    },
                                }),
                                pattern: Box::new(Pattern::Empty),
                            }
                        }]
                    }
                ))
            );
        }

        #[test]
        fn parses_definition() {
            assert_eq!(
                grammar("common.attrib = attribute class { text }"),
                Ok((
                    "",
                    Grammar {
                        contents: vec![GrammarContent::Definition(Definition {
                            name: Identifier {
                                component: "common".into(),
                                sub_components: vec!["attrib".into()],
                            },
                            combine: None,
                            pattern: Pattern::Attribute {
                                name_class: NameClass::Name(Name {
                                    prefix: None,
                                    local: Identifier {
                                        component: "class".into(),
                                        sub_components: vec![],
                                    },
                                }),
                                pattern: Box::new(Pattern::Text),
                            }
                        })]
                    }
                ))
            );
        }

        #[test]
        fn parses_include() {
            assert_eq!(
                grammar("include \"common.rnc\""),
                Ok((
                    "",
                    Grammar {
                        contents: vec![GrammarContent::Include(Include {
                            uri: "common.rnc".into(),
                            inherit: None,
                            grammar: None,
                        })]
                    }
                ))
            );
        }

        #[test]
        fn parses_div() {
            assert_eq!(
                grammar("div { start = empty }"),
                Ok((
                    "",
                    Grammar {
                        contents: vec![GrammarContent::Div(Grammar {
                            contents: vec![GrammarContent::Start {
                                combine: None,
                                pattern: Pattern::Empty,
                            }]
                        })]
                    }
                ))
            );
        }
    }

    mod schema {
        use super::*;
        use pretty_assertions::assert_eq;

        #[test]
        fn parses_pattern_schema() {
            assert_eq!(
                schema("element html { empty }"),
                Ok((
                    "",
                    Schema {
                        declarations: vec![],
                        body: SchemaBody::Pattern(Pattern::Element {
                            name_class: NameClass::Name(Name {
                                prefix: None,
                                local: Identifier {
                                    component: "html".into(),
                                    sub_components: vec![],
                                },
                            }),
                            pattern: Box::new(Pattern::Empty),
                        })
                    }
                ))
            );
        }

        #[test]
        fn parses_grammar_schema() {
            assert_eq!(
                schema("start = empty"),
                Ok((
                    "",
                    Schema {
                        declarations: vec![],
                        body: SchemaBody::Grammar(Grammar {
                            contents: vec![GrammarContent::Start {
                                combine: None,
                                pattern: Pattern::Empty,
                            }]
                        })
                    }
                ))
            );
        }

        #[test]
        fn parses_schema_with_declarations() {
            assert_eq!(
                schema(indoc! {r#"
                    namespace html = "http://www.w3.org/1999/xhtml"
                    start = element html:html { empty }
                "#}),
                Ok((
                    "",
                    Schema {
                        declarations: vec![Declaration::Namespace(NamespaceDeclaration {
                            prefix: Identifier {
                                component: "html".into(),
                                sub_components: vec![],
                            },
                            uri: "http://www.w3.org/1999/xhtml".into(),
                        })],
                        body: SchemaBody::Grammar(Grammar {
                            contents: vec![GrammarContent::Start {
                                combine: None,
                                pattern: Pattern::Element {
                                    name_class: NameClass::Name(Name {
                                        prefix: Some(Identifier {
                                            component: "html".into(),
                                            sub_components: vec![],
                                        }),
                                        local: Identifier {
                                            component: "html".into(),
                                            sub_components: vec![],
                                        },
                                    }),
                                    pattern: Box::new(Pattern::Empty),
                                }
                            }]
                        })
                    }
                ))
            );
        }
    }

    mod annotation {
        use super::*;
        use pretty_assertions::assert_eq;

        #[test]
        fn parses_annotation_attribute() {
            assert_eq!(
                annotation_attribute("a:b = \"c\""),
                Ok((
                    "",
                    AnnotationAttribute {
                        name: Name {
                            prefix: Some(Identifier {
                                component: "a".into(),
                                sub_components: vec![],
                            }),
                            local: Identifier {
                                component: "b".into(),
                                sub_components: vec![],
                            },
                        },
                        value: "c".into(),
                    }
                ))
            );
        }

        #[test]
        fn parses_annotation_element() {
            assert_eq!(
                annotation_element("a:b [ c:d = \"e\" ]"),
                Ok((
                    "",
                    AnnotationElement {
                        name: Name {
                            prefix: Some(Identifier {
                                component: "a".into(),
                                sub_components: vec![],
                            }),
                            local: Identifier {
                                component: "b".into(),
                                sub_components: vec![],
                            },
                        },
                        attributes: vec![AnnotationAttribute {
                            name: Name {
                                prefix: Some(Identifier {
                                    component: "c".into(),
                                    sub_components: vec![],
                                }),
                                local: Identifier {
                                    component: "d".into(),
                                    sub_components: vec![],
                                },
                            },
                            value: "e".into(),
                        }],
                    }
                ))
            );
        }
    }

    mod assignment_operator {
        use super::*;
        use pretty_assertions::assert_eq;

        #[test]
        fn parses_assignment() {
            assert_eq!(assignment_operator("="), Ok(("", None)));
        }

        #[test]
        fn parses_choice_assignment() {
            assert_eq!(assignment_operator("|="), Ok(("", Some(Combine::Choice))));
        }

        #[test]
        fn parses_interleave_assignment() {
            assert_eq!(assignment_operator("&="), Ok(("", Some(Combine::Interleave))));
        }
    }

    mod data_pattern {
        use super::*;
        use pretty_assertions::assert_eq;

        #[test]
        fn parses_data_with_parameters() {
            assert_eq!(
                data_pattern("xsd:string { minLength = \"1\" }"),
                Ok((
                    "",
                    Pattern::Data {
                        name: Name {
                            prefix: Some(Identifier {
                                component: "xsd".into(),
                                sub_components: vec![],
                            }),
                            local: Identifier {
                                component: "string".into(),
                                sub_components: vec![],
                            },
                        },
                        parameters: vec![Parameter {
                            name: Name {
                                prefix: None,
                                local: Identifier {
                                    component: "minLength".into(),
                                    sub_components: vec![],
                                },
                            },
                            value: "1".into(),
                        }],
                        except: None,
                    }
                ))
            );
        }

        #[test]
        fn parses_data_with_except() {
            assert_eq!(
                data_pattern("xsd:string - \"foo\""),
                Ok((
                    "",
                    Pattern::Data {
                        name: Name {
                            prefix: Some(Identifier {
                                component: "xsd".into(),
                                sub_components: vec![],
                            }),
                            local: Identifier {
                                component: "string".into(),
                                sub_components: vec![],
                            },
                        },
                        parameters: vec![],
                        except: Some(Box::new(Pattern::Value {
                            name: None,
                            value: "foo".into(),
                        })),
                    }
                ))
            );
        }

        #[test]
        fn fails_on_plain_name() {
            assert!(data_pattern("xsd:string").is_err());
        }
    }

    mod inherit {
        use super::*;
        use pretty_assertions::assert_eq;

        #[test]
        fn parses_inherit() {
            assert_eq!(
                inherit("inherit = xhtml"),
                Ok((
                    "",
                    Inherit::Prefix(Identifier {
                        component: "xhtml".into(),
                        sub_components: vec![],
                    })
                ))
            );
        }
    }

    mod parameter {
        use super::*;
        use pretty_assertions::assert_eq;

        #[test]
        fn parses_parameter() {
            assert_eq!(
                parameter("minLength = \"1\""),
                Ok((
                    "",
                    Parameter {
                        name: Name {
                            prefix: None,
                            local: Identifier {
                                component: "minLength".into(),
                                sub_components: vec![],
                            },
                        },
                        value: "1".into(),
                    }
                ))
            );
        }
    }

    mod keyword {
        use super::*;
        use pretty_assertions::assert_eq;
        use nom::Parser;

        #[test]
        fn parses_keyword() {
            assert_eq!(
                keyword("element").parse("element "),
                Ok(("", "element"))
            );
        }

        #[test]
        fn fails_if_followed_by_identifier_char() {
            assert!(keyword("element").parse("elemental").is_err());
        }
    }
}
