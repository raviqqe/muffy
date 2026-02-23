use crate::ast::{
    Annotation, AnnotationAttribute, Combine, DatatypesDeclaration, Declaration, Definition,
    Grammar, GrammarItem, Include, Inherit, Name, NameClass, NamespaceDeclaration, Parameter,
    Pattern, Schema, SchemaBody,
};
use nom::{
    IResult, Parser,
    branch::alt,
    bytes::complete::{escaped_transform, is_not, tag, take, take_till},
    character::complete::{alpha1, char, multispace1, satisfy},
    combinator::{all_consuming, map, not, opt, peek, recognize, value, verify},
    error::{Error, ErrorKind},
    multi::{many0, separated_list0, separated_list1},
    sequence::{delimited, preceded, terminated},
};

type ParserError<'input> = Error<&'input str>;

type ParserResult<'input, Output> = IResult<&'input str, Output, ParserError<'input>>;

pub(super) fn schema(input: &str) -> ParserResult<'_, Schema> {
    map(
        blanked((many0(declaration), schema_body)),
        |(declarations, body)| Schema { declarations, body },
    )
    .parse(input)
}

fn schema_body(input: &str) -> ParserResult<'_, SchemaBody> {
    preceded(
        many0(annotation_block),
        alt((
            map(all_consuming(many0(grammar_item)), |items| {
                SchemaBody::Grammar(Grammar { items })
            }),
            map(pattern, SchemaBody::Pattern),
        )),
    )
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
        (
            keyword("namespace"),
            identifier_token,
            symbol("="),
            string_literal,
        ),
        |(_, prefix, _, uri)| NamespaceDeclaration { prefix, uri },
    )
    .parse(input)
}

fn default_namespace_declaration(input: &str) -> ParserResult<'_, String> {
    map(
        (
            keyword("default"),
            keyword("namespace"),
            opt(identifier_token),
            symbol("="),
            string_literal,
        ),
        |(_, _, _, _, uri)| uri,
    )
    .parse(input)
}

fn datatypes_declaration(input: &str) -> ParserResult<'_, DatatypesDeclaration> {
    map(
        (
            keyword("datatypes"),
            opt(identifier_token),
            symbol("="),
            string_literal,
        ),
        |(_, prefix, _, uri)| DatatypesDeclaration { prefix, uri },
    )
    .parse(input)
}

fn grammar(input: &str) -> ParserResult<'_, Grammar> {
    map(
        many0(preceded(many0(annotation_block), grammar_item)),
        |items| Grammar { items },
    )
    .parse(input)
}

fn grammar_item(input: &str) -> ParserResult<'_, GrammarItem> {
    map(
        (
            blanked(alt((
                start_item,
                map(annotation, GrammarItem::Annotation),
                define_item,
                div,
                include,
            ))),
            many0(annotation_block),
        ),
        |(item, _)| item,
    )
    .parse(input)
}

fn start_item(input: &str) -> ParserResult<'_, GrammarItem> {
    map(
        (keyword("start"), assignment_operator, pattern),
        |(_, combine, pattern)| GrammarItem::Start { combine, pattern },
    )
    .parse(input)
}

fn define_item(input: &str) -> ParserResult<'_, GrammarItem> {
    map(
        (identifier_token, assignment_operator, pattern),
        |(name, combine, pattern)| {
            GrammarItem::Definition(Definition {
                name,
                combine,
                pattern,
            })
        },
    )
    .parse(input)
}

fn div(input: &str) -> ParserResult<'_, GrammarItem> {
    map((keyword("div"), braced(grammar)), |(_, grammar)| {
        GrammarItem::Div(grammar)
    })
    .parse(input)
}

fn include(input: &str) -> ParserResult<'_, GrammarItem> {
    map(
        (
            keyword("include"),
            string_literal,
            opt(inherit),
            opt(raw_grammar_block),
        ),
        |(_, uri, inherit, grammar)| {
            GrammarItem::Include(Include {
                uri,
                inherit,
                grammar,
            })
        },
    )
    .parse(input)
}

fn raw_grammar_block(input: &str) -> ParserResult<'_, Grammar> {
    map(braced(raw_grammar_body), |_| Grammar { items: Vec::new() }).parse(input)
}

fn raw_grammar_body(input: &str) -> ParserResult<'_, ()> {
    let mut depth = 0_u32;
    let mut string_delimiter: Option<char> = None;
    let mut escape_next = false;

    for (offset_index, character) in input.char_indices() {
        if let Some(active_delimiter) = string_delimiter {
            if escape_next {
                escape_next = false;
                continue;
            }
            if character == '\\' {
                escape_next = true;
                continue;
            }
            if character == active_delimiter {
                string_delimiter = None;
            }
            continue;
        }

        match character {
            '"' | '\'' => string_delimiter = Some(character),
            '{' => depth += 1,
            '}' => {
                if depth == 0 {
                    let remaining_input = &input[offset_index..];
                    return Ok((remaining_input, ()));
                }
                depth = depth.saturating_sub(1);
            }
            _ => {}
        }
    }

    Err(nom::Err::Error(Error::new(input, ErrorKind::Tag)))
}

fn inherit(input: &str) -> ParserResult<'_, Inherit> {
    map(
        (keyword("inherit"), preceded(symbol("="), identifier_token)),
        |(_, prefix)| Inherit::Prefix(prefix),
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
    choice_pattern(input)
}

fn choice_pattern(input: &str) -> ParserResult<'_, Pattern> {
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
            many0(annotation_attachment),
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
            // TODO Return an error on an invalid quantifier.
            Some(_) | None => pattern,
        },
    )
    .parse(input)
}

fn annotation_attachment(input: &str) -> ParserResult<'_, ()> {
    map((symbol(">>"), annotation), |_| ()).parse(input)
}

fn primary_pattern(input: &str) -> ParserResult<'_, Pattern> {
    preceded(
        many0(annotation_block),
        alt((
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
        )),
    )
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
    map((keyword("list"), braced(pattern)), |(_, pattern)| {
        Pattern::List(pattern.into())
    })
    .parse(input)
}

fn grammar_pattern(input: &str) -> ParserResult<'_, Pattern> {
    map((keyword("grammar"), braced(grammar)), |(_, grammar)| {
        Pattern::Grammar(grammar)
    })
    .parse(input)
}

fn external_pattern(input: &str) -> ParserResult<'_, Pattern> {
    map((keyword("external"), string_literal), |(_, uri)| {
        Pattern::ExternalRef(uri)
    })
    .parse(input)
}

fn text_pattern(input: &str) -> ParserResult<'_, Pattern> {
    map(keyword("text"), |_| Pattern::Text).parse(input)
}

fn empty_pattern(input: &str) -> ParserResult<'_, Pattern> {
    map(keyword("empty"), |_| Pattern::Empty).parse(input)
}

fn not_allowed_pattern(input: &str) -> ParserResult<'_, Pattern> {
    map(keyword("notAllowed"), |_| Pattern::NotAllowed).parse(input)
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
    map((opt(name), string_literal), |(datatype_name, value)| {
        Pattern::Value {
            name: datatype_name,
            value,
        }
    })
    .parse(input)
}

fn name_class(input: &str) -> ParserResult<'_, NameClass> {
    name_class_choice(input)
}

fn name_class_choice(input: &str) -> ParserResult<'_, NameClass> {
    map(separated_list1(symbol("|"), name_class_except), |classes| {
        if classes.len() == 1 {
            classes
                .into_iter()
                .next()
                .expect("name class list must contain one item")
        } else {
            NameClass::Choice(classes)
        }
    })
    .parse(input)
}

fn name_class_except(input: &str) -> ParserResult<'_, NameClass> {
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
        map((identifier, char(':'), char('*')), |(prefix, _, _)| {
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
    map(
        (name, preceded(symbol("="), string_literal)),
        |(name, value)| Parameter { name, value },
    )
    .parse(input)
}

fn annotation(input: &str) -> ParserResult<'_, Annotation> {
    map((name, annotation_block), |(name, attributes)| Annotation {
        name,
        attributes,
    })
    .parse(input)
}

fn annotation_block(input: &str) -> ParserResult<'_, Vec<AnnotationAttribute>> {
    alt((annotation_block_attributes, annotation_block_raw)).parse(input)
}

fn annotation_block_attributes(input: &str) -> ParserResult<'_, Vec<AnnotationAttribute>> {
    map(
        bracketed((many0(annotation_attribute), blank)),
        |(attributes, _)| attributes,
    )
    .parse(input)
}

fn annotation_block_raw(input: &str) -> ParserResult<'_, Vec<AnnotationAttribute>> {
    map(bracketed(annotation_block_body), |_| Vec::new()).parse(input)
}

fn annotation_block_body(input: &str) -> ParserResult<'_, ()> {
    let mut depth = 0_u32;
    let mut string_delimiter: Option<char> = None;
    let mut escape_next = false;

    for (offset_index, character) in input.char_indices() {
        if let Some(active_delimiter) = string_delimiter {
            if escape_next {
                escape_next = false;
                continue;
            }
            if character == '\\' {
                escape_next = true;
                continue;
            }
            if character == active_delimiter {
                string_delimiter = None;
            }
            continue;
        }

        match character {
            '"' | '\'' => string_delimiter = Some(character),
            '[' => depth += 1,
            ']' => {
                if depth == 0 {
                    let remaining_input = &input[offset_index..];
                    return Ok((remaining_input, ()));
                }
                depth = depth.saturating_sub(1);
            }
            _ => {}
        }
    }

    Err(nom::Err::Error(Error::new(input, ErrorKind::Tag)))
}

fn annotation_attribute(input: &str) -> ParserResult<'_, AnnotationAttribute> {
    map(
        (name, preceded(symbol("="), string_literal)),
        |(name, value)| AnnotationAttribute { name, value },
    )
    .parse(input)
}

fn identifier_token(input: &str) -> ParserResult<'_, String> {
    blanked(identifier).parse(input)
}

fn name(input: &str) -> ParserResult<'_, Name> {
    map(
        blanked((identifier, opt(preceded(char(':'), identifier)))),
        |(first, rest)| {
            let (prefix, local) = match rest {
                Some(local) => (Some(first), local),
                None => (None, first),
            };
            Name { prefix, local }
        },
    )
    .parse(input)
}

fn string_literal(input: &str) -> ParserResult<'_, String> {
    blanked(alt((
        map(
            delimited(
                char('"'),
                opt(escaped_transform(is_not("\\\""), '\\', string_escape)),
                char('"'),
            ),
            |value| value.unwrap_or_default(),
        ),
        map(
            delimited(
                char('\''),
                opt(escaped_transform(is_not("\\'"), '\\', string_escape)),
                char('\''),
            ),
            |value| value.unwrap_or_default(),
        ),
    )))
    .parse(input)
}

fn keyword(keyword: &'static str) -> impl Fn(&str) -> ParserResult<'_, &str> {
    move |input| {
        delimited(
            blank,
            terminated(tag(keyword), not(peek(satisfy(is_identifier_char)))),
            blank,
        )
        .parse(input)
    }
}

fn symbol(symbol: &'static str) -> impl Fn(&str) -> ParserResult<'_, &str> {
    move |input| blanked(tag(symbol)).parse(input)
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
    map(many0(alt((value((), multispace1), comment))), |_| ()).parse(input)
}

fn comment(input: &str) -> ParserResult<'_, ()> {
    map(
        (
            preceded(tag("#"), take_till(|character| character == '\n')),
            opt(char('\n')),
        ),
        |_| (),
    )
    .parse(input)
}

fn identifier(input: &str) -> ParserResult<'_, String> {
    map(
        preceded(
            opt(char::<&str, _>('\\')),
            recognize((alpha1, many0(satisfy(is_identifier_char)))),
        ),
        Into::into,
    )
    .parse(input)
}

const fn is_identifier_char(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == '_' || character == '-' || character == '.'
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

fn fold_patterns(patterns: Vec<Pattern>, constructor: fn(Vec<Pattern>) -> Pattern) -> Pattern {
    if patterns.len() == 1 {
        patterns
            .into_iter()
            .next()
            .expect("pattern list must contain one item")
    } else {
        constructor(patterns)
    }
}

#[cfg(test)]
mod tests {
    use super::super::parse_schema;
    use super::*;
    use indoc::indoc;

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

        let schema = parse_schema(input).expect("schema should parse");

        let expected = Schema {
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
        };

        assert_eq!(schema, expected);
    }

    #[test]
    fn parse_grammar_schema_with_definitions() {
        let input = indoc! {r#"
            namespace sch = "http://example.com/sch"

            sch:ns [ prefix = "html" uri = "http://example.com/ns" ]
            start = element html { empty }
            common &= element div { text }
        "#};

        let schema = parse_schema(input).expect("schema should parse");

        let expected = Schema {
            declarations: vec![Declaration::Namespace(NamespaceDeclaration {
                prefix: "sch".to_string(),
                uri: "http://example.com/sch".to_string(),
            })],
            body: SchemaBody::Grammar(Grammar {
                items: vec![
                    GrammarItem::Annotation(Annotation {
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
                    GrammarItem::Start {
                        combine: None,
                        pattern: Pattern::Element {
                            name_class: NameClass::Name(local_name("html")),
                            pattern: Box::new(Pattern::Empty),
                        },
                    },
                    GrammarItem::Definition(Definition {
                        name: "common".to_string(),
                        combine: Some(Combine::Interleave),
                        pattern: Pattern::Element {
                            name_class: NameClass::Name(local_name("div")),
                            pattern: Box::new(Pattern::Text),
                        },
                    }),
                ],
            }),
        };

        assert_eq!(schema, expected);
    }

    #[test]
    fn parse_data_and_value_patterns() {
        let input = "attribute size { xsd:integer { minInclusive = \"1\" } - \"5\" }";

        let schema = parse_schema(input).expect("schema should parse");

        let expected = Schema {
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
        };

        assert_eq!(schema, expected);
    }

    #[test]
    fn parse_name_class_except() {
        let input = "element (* - (foo | bar)) { text }";

        let schema = parse_schema(input).expect("schema should parse");

        let expected = Schema {
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
        };

        assert_eq!(schema, expected);
    }

    #[test]
    fn parse_annotation_item() {
        let input = "sch:ns [ prefix = \"html\" uri = \"http://example.com/ns\" ]";

        let annotation_result = super::annotation(input);
        assert!(
            annotation_result.is_ok(),
            "annotation element parse failed: {annotation_result:?}"
        );
        let result = super::grammar_item(input);

        assert!(
            result.is_ok(),
            "annotation element parse failed: {result:?}"
        );
    }

    #[test]
    fn parse_basic_table_definition() {
        let input = "table = element table { table.attlist, caption?, tr+ }";

        let result = super::grammar_item(input);

        assert!(result.is_ok(), "definition parse failed: {result:?}");
    }

    #[test]
    fn parse_prefixed_attribute_pattern() {
        let input = "attribute xml:space { string \"preserve\" }?";

        let result = super::pattern(input);

        assert!(
            result.is_ok(),
            "prefixed attribute parse failed: {result:?}"
        );
    }

    #[test]
    fn parse_definition_with_inline_comment() {
        let input = indoc! {r#"
            xml.space.attrib = # added -- hsivonen
                attribute xml:space { string "preserve" }?
        "#};

        let result = super::grammar_item(input);

        assert!(result.is_ok(), "inline comment parse failed: {result:?}");
    }

    #[test]
    fn parse_definition_followed_by_next_item() {
        let input = indoc! {r#"
            xml.space.attrib = # added -- hsivonen
                attribute xml:space { string "preserve" }?
            class.attrib = attribute class { text }?
        "#};

        let (remaining_input, _) = super::grammar_item(input).expect("definition should parse");

        assert!(
            remaining_input.trim_start().starts_with("class.attrib"),
            "expected to stop at next definition, got: {remaining_input:?}"
        );
    }

    #[test]
    fn parse_prefixed_name_class() {
        let input = "xml:space { string \"preserve\" }";

        let result = super::name_class(input);

        assert!(result.is_ok(), "prefixed name class failed: {result:?}");
    }

    #[test]
    fn parse_attribute_pattern_followed_by_next_item() {
        let input = indoc! {r#"
            attribute xml:space { string "preserve" }?
            class.attrib = attribute class { text }?
        "#};

        let (remaining_input, _) = super::pattern(input).expect("attribute pattern should parse");

        assert!(
            remaining_input.trim_start().starts_with("class.attrib"),
            "expected to stop at next definition, got: {remaining_input:?}"
        );
    }

    #[test]
    fn parse_attribute_pattern_parser() {
        let input = indoc! {r#"
            attribute xml:space { string "preserve" }?
            class.attrib = attribute class { text }?
        "#};

        let (remaining_input, _) = super::attribute_pattern(input).expect("attribute should parse");

        assert!(
            remaining_input.trim_start().starts_with("?"),
            "expected quantifier, got: {remaining_input:?}"
        );
    }

    #[test]
    fn parse_default_namespace_with_prefix() {
        let input = indoc! {r#"
            default namespace svg = "http://example.com"

            element svg { empty }
        "#};

        let result = parse_schema(input);

        assert!(result.is_ok(), "default namespace parse failed: {result:?}");
    }

    #[test]
    fn parse_empty_schema() {
        let input = "# comment only";

        let result = parse_schema(input);

        assert!(result.is_ok(), "empty schema parse failed: {result:?}");
    }

    #[test]
    fn parse_annotation_attachment() {
        let input = "element pre { pre.attlist >> sch:pattern [ name = \"pre\" ] , Inline.model }";

        let result = parse_schema(input);

        assert!(
            result.is_ok(),
            "annotation attachment parse failed: {result:?}"
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

        let result = parse_schema(input);

        assert!(
            result.is_ok(),
            "include annotation parse failed: {result:?}"
        );
    }

    #[test]
    fn parse_element_with_parenthesized_choice() {
        let input = "element select { select.attlist, (option | optgroup)+ }";

        let result = parse_schema(input);

        assert!(
            result.is_ok(),
            "parenthesized choice parse failed: {result:?}"
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

        let result = parse_schema(input);

        assert!(
            result.is_ok(),
            "nested annotation include failed: {result:?}"
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

        let result = parse_schema(input);

        assert!(
            result.is_ok(),
            "annotation attachment comment failed: {result:?}"
        );
    }

    #[test]
    fn parse_grammar_with_leading_annotation_block() {
        let input = "[ sch:pattern [ name = \"select.multiple\" ] ] select = element select { select.attlist, (option | optgroup)+ }";

        let (remaining_input, _) = super::grammar(input).expect("grammar should parse");

        assert!(
            remaining_input.trim_start().is_empty(),
            "expected grammar to consume input, got: {remaining_input:?}"
        );
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

        let result = parse_schema(input);

        assert!(
            result.is_ok(),
            "bracketed annotation include failed: {result:?}"
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

        let result = parse_schema(input);

        assert!(
            result.is_ok(),
            "schematron include parse failed: {result:?}"
        );
    }

    #[test]
    fn parse_include_block_as_grammar_item() {
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

        let (remaining_input, _) = super::grammar_item(input).expect("include item should parse");

        assert!(
            remaining_input.trim_start().starts_with("form.attlist"),
            "expected to stop at form.attlist, got: {remaining_input:?}"
        );
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

        let (remaining_input, _) = super::raw_grammar_block(input).expect("raw block should parse");

        assert!(
            remaining_input.trim_start().starts_with("form.attlist"),
            "expected to stop at form.attlist, got: {remaining_input:?}"
        );
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

        let result = parse_schema(input);

        assert!(
            result.is_ok(),
            "annotation include parse failed: {result:?}"
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

        let (remaining_input, _) =
            super::raw_grammar_block(input).expect("raw include should parse");

        assert!(
            remaining_input.trim_start().starts_with("form.attlist"),
            "expected to stop at form.attlist, got: {remaining_input:?}"
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

        let result = super::grammar_item(input);

        assert!(result.is_ok(), "choice with comment failed: {result:?}");
    }
}
