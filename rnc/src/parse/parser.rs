use crate::ast::{
    AnnotationAttribute, AnnotationElement, Combine, DatatypesDeclaration, Declaration, Definition,
    Grammar, GrammarContent, Include, Inherit, Name, NameClass, NamespaceDeclaration, Parameter,
    Pattern, Schema, SchemaBody,
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

fn identifier(input: &str) -> ParserResult<'_, String> {
    blanked(raw_identifier).parse(input)
}

fn raw_identifier(input: &str) -> ParserResult<'_, String> {
    map(
        preceded(
            opt(char('\\')),
            recognize((alpha1, many0(satisfy(is_identifier_char)))),
        ),
        Into::into,
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
    use super::{super::parse_schema, *};
    use indoc::indoc;
    use pretty_assertions::assert_eq;

    fn local_name(value: &str) -> Name {
        Name {
            prefix: None,
            local: value.to_string(),
        }
    }

    fn prefixed_name(prefix: &str, value: &str) -> Name {
        Name {
            prefix: Some(prefix.to_string()),
            local: value.to_string(),
        }
    }

    #[test]
    fn parse_pattern_schema() {
        let input = "element foo { (text | empty)+, attribute id { text }? }";

        assert_eq!(
            parse_schema(input).unwrap(),
            Schema {
                declarations: Vec::new(),
                body: SchemaBody::Pattern(Pattern::Element {
                    name_class: NameClass::Name(local_name("foo")),
                    pattern: Box::new(Pattern::Group(vec![
                        Pattern::Many1(Box::new(Pattern::Choice(vec![
                            Pattern::Text,
                            Pattern::Empty,
                        ]))),
                        Pattern::Optional(Box::new(Pattern::Attribute {
                            name_class: NameClass::Name(local_name("id")),
                            pattern: Box::new(Pattern::Text),
                        })),
                    ])),
                }),
            }
        );
    }

    #[test]
    fn parse_grammar_schema_with_definitions() {
        let input = indoc! {r#"
            namespace sch = "http://example.com/sch"

            sch:ns [ prefix = "html" uri = "http://example.com/ns" ]
            start = element html { empty }
            common &= element div { text }
        "#};

        assert_eq!(
            parse_schema(input).unwrap(),
            Schema {
                declarations: vec![Declaration::Namespace(NamespaceDeclaration {
                    prefix: "sch".to_string(),
                    uri: "http://example.com/sch".to_string(),
                })],
                body: SchemaBody::Grammar(Grammar {
                    contents: vec![
                        GrammarContent::Annotation(AnnotationElement {
                            name: prefixed_name("sch", "ns"),
                            attributes: vec![
                                AnnotationAttribute {
                                    name: local_name("prefix"),
                                    value: "html".to_string(),
                                },
                                AnnotationAttribute {
                                    name: local_name("uri"),
                                    value: "http://example.com/ns".to_string(),
                                },
                            ],
                        }),
                        GrammarContent::Start {
                            combine: None,
                            pattern: Pattern::Element {
                                name_class: NameClass::Name(local_name("html")),
                                pattern: Box::new(Pattern::Empty),
                            },
                        },
                        GrammarContent::Definition(Definition {
                            name: "common".to_string(),
                            combine: Some(Combine::Interleave),
                            pattern: Pattern::Element {
                                name_class: NameClass::Name(local_name("div")),
                                pattern: Box::new(Pattern::Text),
                            },
                        }),
                    ],
                }),
            }
        );
    }

    #[test]
    fn parse_data_and_value_patterns() {
        let input = "attribute size { xsd:integer { minInclusive = \"1\" } - \"5\" }";

        assert_eq!(
            parse_schema(input).unwrap(),
            Schema {
                declarations: Vec::new(),
                body: SchemaBody::Pattern(Pattern::Attribute {
                    name_class: NameClass::Name(local_name("size")),
                    pattern: Box::new(Pattern::Data {
                        name: prefixed_name("xsd", "integer"),
                        parameters: vec![Parameter {
                            name: local_name("minInclusive"),
                            value: "1".to_string(),
                        }],
                        except: Some(Box::new(Pattern::Value {
                            name: None,
                            value: "5".to_string(),
                        })),
                    }),
                }),
            }
        );
    }

    #[test]
    fn parse_name_class_except() {
        let input = "element (* - (foo | bar)) { text }";

        assert_eq!(
            parse_schema(input).unwrap(),
            Schema {
                declarations: Vec::new(),
                body: SchemaBody::Pattern(Pattern::Element {
                    name_class: NameClass::Except {
                        base: Box::new(NameClass::AnyName),
                        except: Box::new(NameClass::Choice(vec![
                            NameClass::Name(local_name("foo")),
                            NameClass::Name(local_name("bar")),
                        ])),
                    },
                    pattern: Box::new(Pattern::Text),
                }),
            }
        );
    }

    #[test]
    fn parse_annotation_item() {
        let input = "sch:ns [ prefix = \"html\" uri = \"http://example.com/ns\" ]";

        assert_eq!(
            annotation_element(input).unwrap(),
            (
                "",
                AnnotationElement {
                    name: prefixed_name("sch", "ns"),
                    attributes: vec![
                        AnnotationAttribute {
                            name: local_name("prefix"),
                            value: "html".to_string(),
                        },
                        AnnotationAttribute {
                            name: local_name("uri"),
                            value: "http://example.com/ns".to_string(),
                        },
                    ],
                }
            )
        );
        assert_eq!(
            grammar_content(input).unwrap(),
            (
                "",
                GrammarContent::Annotation(AnnotationElement {
                    name: prefixed_name("sch", "ns"),
                    attributes: vec![
                        AnnotationAttribute {
                            name: local_name("prefix"),
                            value: "html".to_string(),
                        },
                        AnnotationAttribute {
                            name: local_name("uri"),
                            value: "http://example.com/ns".to_string(),
                        },
                    ],
                })
            )
        );
    }

    #[test]
    fn parse_basic_table_definition() {
        let input = "table = element table { table.attlist, caption?, tr+ }";

        assert_eq!(
            grammar_content(input).unwrap(),
            (
                "",
                GrammarContent::Definition(Definition {
                    name: "table".to_string(),
                    combine: None,
                    pattern: Pattern::Element {
                        name_class: NameClass::Name(local_name("table")),
                        pattern: Box::new(Pattern::Group(vec![
                            Pattern::Name(local_name("table.attlist")),
                            Pattern::Optional(Box::new(Pattern::Name(local_name("caption")))),
                            Pattern::Many1(Box::new(Pattern::Name(local_name("tr")))),
                        ])),
                    },
                })
            )
        );
    }

    #[test]
    fn parse_prefixed_attribute_pattern() {
        let input = "attribute xml:space { string \"preserve\" }?";

        assert_eq!(
            pattern(input).unwrap(),
            (
                "",
                Pattern::Optional(Box::new(Pattern::Attribute {
                    name_class: NameClass::Name(prefixed_name("xml", "space")),
                    pattern: Box::new(Pattern::Value {
                        name: Some(local_name("string")),
                        value: "preserve".to_string(),
                    }),
                }))
            )
        );
    }

    #[test]
    fn parse_definition_with_inline_comment() {
        let input = indoc! {r#"
            xml.space.attrib = # added -- hsivonen
                attribute xml:space { string "preserve" }?
        "#};

        assert_eq!(
            grammar_content(input).unwrap(),
            (
                "",
                GrammarContent::Definition(Definition {
                    name: "xml.space.attrib".to_string(),
                    combine: None,
                    pattern: Pattern::Optional(Box::new(Pattern::Attribute {
                        name_class: NameClass::Name(prefixed_name("xml", "space")),
                        pattern: Box::new(Pattern::Value {
                            name: Some(local_name("string")),
                            value: "preserve".to_string(),
                        }),
                    })),
                })
            )
        );
    }

    #[test]
    fn parse_definition_followed_by_next_item() {
        let input = indoc! {r#"
            xml.space.attrib = # added -- hsivonen
                attribute xml:space { string "preserve" }?
            class.attrib = attribute class { text }?
        "#};

        let (remaining_input, _) = grammar_content(input).unwrap();

        assert!(remaining_input.trim_start().starts_with("class.attrib"));
    }

    #[test]
    fn parse_prefixed_name_class() {
        let input = "xml:space { string \"preserve\" }";

        assert_eq!(
            name_class(input).unwrap(),
            (
                "{ string \"preserve\" }",
                NameClass::Name(prefixed_name("xml", "space"))
            )
        );
    }

    #[test]
    fn parse_attribute_pattern_followed_by_next_item() {
        let input = indoc! {r#"
            attribute xml:space { string "preserve" }?
            class.attrib = attribute class { text }?
        "#};

        let (remaining_input, _) = pattern(input).unwrap();

        assert!(remaining_input.trim_start().starts_with("class.attrib"));
    }

    #[test]
    fn parse_attribute_pattern_parser() {
        let input = indoc! {r#"
            attribute xml:space { string "preserve" }?
            class.attrib = attribute class { text }?
        "#};

        let (remaining_input, _) = attribute_pattern(input).unwrap();

        assert!(remaining_input.trim_start().starts_with("?"));
    }

    #[test]
    fn parse_default_namespace_with_prefix() {
        let input = indoc! {r#"
            default namespace svg = "http://example.com"

            element svg { empty }
        "#};

        assert_eq!(
            parse_schema(input).unwrap(),
            Schema {
                declarations: vec![Declaration::DefaultNamespace(
                    "http://example.com".to_string()
                )],
                body: SchemaBody::Pattern(Pattern::Element {
                    name_class: NameClass::Name(local_name("svg")),
                    pattern: Box::new(Pattern::Empty),
                }),
            }
        );
    }

    #[test]
    fn parse_empty_schema() {
        let input = "# comment only";

        assert_eq!(
            parse_schema(input).unwrap(),
            Schema {
                declarations: Vec::new(),
                body: SchemaBody::Grammar(Grammar {
                    contents: Vec::new()
                }),
            }
        );
    }

    #[test]
    fn parse_annotation_attachment() {
        let input = "element pre { pre.attlist >> sch:pattern [ name = \"pre\" ] , Inline.model }";

        assert_eq!(
            parse_schema(input).unwrap(),
            Schema {
                declarations: Vec::new(),
                body: SchemaBody::Pattern(Pattern::Element {
                    name_class: NameClass::Name(local_name("pre")),
                    pattern: Box::new(Pattern::Group(vec![
                        Pattern::Name(local_name("pre.attlist")),
                        Pattern::Name(local_name("Inline.model")),
                    ])),
                }),
            }
        );
    }

    #[test]
    fn parse_include_with_annotation_block() {
        let input = indoc! {r#"
            include "base.rnc" {
                [ sch:pattern [ name = "x" ] ]
                start = empty
            }
        "#};

        assert_eq!(
            parse_schema(input).unwrap(),
            Schema {
                declarations: vec![],
                body: SchemaBody::Grammar(Grammar {
                    contents: vec![GrammarContent::Include(Include {
                        uri: "base.rnc".to_string(),
                        inherit: None,
                        grammar: Some(Grammar {
                            contents: vec![GrammarContent::Start {
                                combine: None,
                                pattern: Pattern::Empty,
                            }],
                        }),
                    })],
                }),
            }
        );
    }

    #[test]
    fn parse_element_with_parenthesized_choice() {
        let input = "element select { select.attlist, (option | optgroup)+ }";

        assert_eq!(
            parse_schema(input).unwrap(),
            Schema {
                declarations: Vec::new(),
                body: SchemaBody::Pattern(Pattern::Element {
                    name_class: NameClass::Name(local_name("select")),
                    pattern: Box::new(Pattern::Group(vec![
                        Pattern::Name(local_name("select.attlist")),
                        Pattern::Many1(Box::new(Pattern::Choice(vec![
                            Pattern::Name(local_name("option")),
                            Pattern::Name(local_name("optgroup")),
                        ]))),
                    ])),
                }),
            }
        );
    }

    #[test]
    fn parse_include_with_nested_annotation_block() {
        let input = indoc! {r#"
            include "basic-form.rnc" {
                [
                    sch:pattern [
                        name = "select.multiple"
                        "\x{a}"
                    ]
                ]
                select = element select { select.attlist, (option | optgroup)+ }
            }
        "#};

        assert_eq!(
            parse_schema(input).unwrap(),
            Schema {
                declarations: vec![],
                body: SchemaBody::Grammar(Grammar {
                    contents: vec![GrammarContent::Include(Include {
                        uri: "basic-form.rnc".to_string(),
                        inherit: None,
                        grammar: Some(Grammar {
                            contents: vec![GrammarContent::Definition(Definition {
                                name: "select".to_string(),
                                combine: None,
                                pattern: Pattern::Element {
                                    name_class: NameClass::Name(local_name("select")),
                                    pattern: Box::new(Pattern::Group(vec![
                                        Pattern::Name(local_name("select.attlist")),
                                        Pattern::Many1(Box::new(Pattern::Choice(vec![
                                            Pattern::Name(local_name("option")),
                                            Pattern::Name(local_name("optgroup")),
                                        ]))),
                                    ])),
                                },
                            }),],
                        }),
                    })],
                }),
            }
        );
    }

    #[test]
    fn parse_annotation_attachment_with_comment() {
        let input = indoc! {r#"
            element button {
                button.attlist,
                Flow.model
                # comment
                >> sch:pattern [ name = "button.content" ]
            }
        "#};

        assert_eq!(
            parse_schema(input).unwrap(),
            Schema {
                declarations: Vec::new(),
                body: SchemaBody::Pattern(Pattern::Element {
                    name_class: NameClass::Name(local_name("button")),
                    pattern: Box::new(Pattern::Group(vec![
                        Pattern::Name(local_name("button.attlist")),
                        Pattern::Name(local_name("Flow.model")),
                    ])),
                }),
            }
        );
    }

    #[test]
    fn parse_grammar_with_leading_annotation_block() {
        let input = indoc! {r#"
            [ sch:pattern [ name = "select.multiple" ] ] select =
                element select { select.attlist, (option | optgroup)+ }
        "#};

        let (remaining_input, _) = grammar(input).unwrap();

        assert!(remaining_input.trim_start().is_empty());
    }

    #[test]
    fn parse_include_with_nested_annotation_and_brackets() {
        let input = indoc! {r#"
            include "basic-form.rnc" {
                [
                    sch:pattern [
                        name = "select.multiple"
                        sch:report [
                            test = "html:option[@selected]"
                        ]
                    ]
                ]
                select = element select { select.attlist, (option | optgroup)+ }
            }
        "#};

        assert_eq!(
            parse_schema(input).unwrap(),
            Schema {
                declarations: vec![],
                body: SchemaBody::Grammar(Grammar {
                    contents: vec![GrammarContent::Include(Include {
                        uri: "basic-form.rnc".to_string(),
                        inherit: None,
                        grammar: Some(Grammar {
                            contents: vec![GrammarContent::Definition(Definition {
                                name: "select".to_string(),
                                combine: None,
                                pattern: Pattern::Element {
                                    name_class: NameClass::Name(local_name("select")),
                                    pattern: Box::new(Pattern::Group(vec![
                                        Pattern::Name(local_name("select.attlist")),
                                        Pattern::Many1(Box::new(Pattern::Choice(vec![
                                            Pattern::Name(local_name("option")),
                                            Pattern::Name(local_name("optgroup")),
                                        ]))),
                                    ])),
                                },
                            }),],
                        }),
                    })],
                }),
            }
        );
    }

    #[test]
    fn parse_include_with_schematron_block() {
        let input = indoc! {r#"
            include "basic-form.rnc" {
                [
                    sch:pattern [
                        name = "select.multiple.selected.options"
                        sch:report [
                            test = "not(@multiple) and count(html:option[@selected]) > 1"
                        ]
                    ]
                ]
                select = element select { select.attlist, (option | optgroup)+ }
            }
        "#};

        assert_eq!(
            parse_schema(input).unwrap(),
            Schema {
                declarations: vec![],
                body: SchemaBody::Grammar(Grammar {
                    contents: vec![GrammarContent::Include(Include {
                        uri: "basic-form.rnc".to_string(),
                        inherit: None,
                        grammar: Some(Grammar {
                            contents: vec![GrammarContent::Definition(Definition {
                                name: "select".to_string(),
                                combine: None,
                                pattern: Pattern::Element {
                                    name_class: NameClass::Name(local_name("select")),
                                    pattern: Box::new(Pattern::Group(vec![
                                        Pattern::Name(local_name("select.attlist")),
                                        Pattern::Many1(Box::new(Pattern::Choice(vec![
                                            Pattern::Name(local_name("option")),
                                            Pattern::Name(local_name("optgroup")),
                                        ]))),
                                    ])),
                                },
                            }),],
                        }),
                    })],
                }),
            }
        );
    }

    #[test]
    fn parse_include_block_as_grammar_content() {
        let input = indoc! {r#"
            include "basic-form.rnc" {
                [
                    sch:pattern [
                        name = "select.multiple.selected.options"
                        sch:report [
                            test = "not(@multiple) and count(html:option[@selected]) > 1"
                        ]
                    ]
                ]
                select = element select { select.attlist, (option | optgroup)+ }
            }
            form.attlist &= attribute accept-charset { charsets.datatype }?
        "#};

        let (remaining_input, _) = grammar_content(input).unwrap();

        assert!(remaining_input.trim_start().starts_with("form.attlist"));
    }

    #[test]
    fn parse_raw_include_block() {
        let input = indoc! {r#"
            {
                [
                    sch:pattern [
                        name = "select.multiple.selected.options"
                        sch:report [
                            test = "not(@multiple) and count(html:option[@selected]) > 1"
                        ]
                    ]
                ]
                select = element select { select.attlist, (option | optgroup)+ }
            }
            form.attlist &= attribute accept-charset { charsets.datatype }?
        "#};

        let (remaining_input, _) = braced(grammar).parse(input).unwrap();

        assert!(remaining_input.trim_start().starts_with("form.attlist"));
    }

    #[test]
    fn parse_annotation_before_include_block() {
        let input = indoc! {r#"
            sch:ns [ prefix = "html" uri = "http://www.w3.org/1999/xhtml" ]
            include "basic-form.rnc" {
                [
                    sch:pattern [
                        name = "select.multiple.selected.options"
                        sch:report [
                            test = "not(@multiple) and count(html:option[@selected]) > 1"
                        ]
                    ]
                ]
                select = element select { select.attlist, (option | optgroup)+ }
            }
            form.attlist &= attribute accept-charset { charsets.datatype }?
        "#};

        assert_eq!(
            parse_schema(input).unwrap(),
            Schema {
                declarations: Vec::new(),
                body: SchemaBody::Grammar(Grammar {
                    contents: vec![
                        GrammarContent::Annotation(AnnotationElement {
                            name: prefixed_name("sch", "ns"),
                            attributes: vec![
                                AnnotationAttribute {
                                    name: local_name("prefix"),
                                    value: "html".to_string(),
                                },
                                AnnotationAttribute {
                                    name: local_name("uri"),
                                    value: "http://www.w3.org/1999/xhtml".to_string(),
                                },
                            ],
                        }),
                        GrammarContent::Include(Include {
                            uri: "basic-form.rnc".to_string(),
                            inherit: None,
                            grammar: Some(Grammar {
                                contents: vec![GrammarContent::Definition(Definition {
                                    name: "select".to_string(),
                                    combine: None,
                                    pattern: Pattern::Element {
                                        name_class: NameClass::Name(local_name("select")),
                                        pattern: Box::new(Pattern::Group(vec![
                                            Pattern::Name(local_name("select.attlist")),
                                            Pattern::Many1(Box::new(Pattern::Choice(vec![
                                                Pattern::Name(local_name("option")),
                                                Pattern::Name(local_name("optgroup")),
                                            ]))),
                                        ])),
                                    },
                                }),],
                            }),
                        }),
                        GrammarContent::Definition(Definition {
                            name: "form.attlist".to_string(),
                            combine: Some(Combine::Interleave),
                            pattern: Pattern::Optional(Box::new(Pattern::Attribute {
                                name_class: NameClass::Name(local_name("accept-charset")),
                                pattern: Box::new(Pattern::Name(local_name("charsets.datatype"))),
                            })),
                        }),
                    ],
                }),
            }
        );
    }

    #[test]
    fn parse_raw_include_block_with_escape_sequences() {
        let input = indoc! {r#"
            {
                [
                    sch:pattern [
                        name = "select.multiple.selected.options"
                        "\x{a}" ~
                        "          "
                        sch:rule [
                            context = "html:select"
                            "\x{a}" ~
                            "              "
                            sch:report [
                                test =
                                    "not(@multiple) and count(html:option[@selected]) > 1"
                                "\x{a}" ~
                                "                   Select elements which aren't marked as multiple may not have more then one selected option.\x{a}" ~
                                "              "
                            ]
                            "\x{a}" ~
                            "          "
                        ]
                        "\x{a}" ~
                        "      "
                    ]
                ]
                select = element select { select.attlist, (option | optgroup)+ }
            }
            form.attlist &= attribute accept-charset { charsets.datatype }?
        "#};

        assert!(
            braced(grammar)
                .parse(input)
                .unwrap()
                .0
                .trim_start()
                .starts_with("form.attlist")
        );
    }

    #[test]
    fn parse_choice_with_inline_comment() {
        let input = indoc! {r#"
            InputType.class |=
                string "image"
                | string "button"
                | # bugfix
                  string "file"
        "#};

        assert_eq!(
            grammar_content(input).unwrap(),
            (
                "",
                GrammarContent::Definition(Definition {
                    name: "InputType.class".to_string(),
                    combine: Some(Combine::Choice),
                    pattern: Pattern::Choice(vec![
                        Pattern::Value {
                            name: Some(local_name("string")),
                            value: "image".to_string(),
                        },
                        Pattern::Value {
                            name: Some(local_name("string")),
                            value: "button".to_string(),
                        },
                        Pattern::Value {
                            name: Some(local_name("string")),
                            value: "file".to_string(),
                        },
                    ]),
                })
            )
        );
    }
}
