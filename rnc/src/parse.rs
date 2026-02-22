//! Parser implementation for Relax NG Compact Syntax.

use crate::ast::{
    Annotation, AnnotationAttribute, Combine, DatatypesDeclaration, Declaration, Definition,
    Grammar, GrammarItem, Include, Inherit, Name, NameClass, NamespaceDeclaration, Parameter,
    Pattern, Schema, SchemaBody,
};
use core::fmt::{self, Display, Formatter};
use nom::{
    IResult, Parser,
    branch::alt,
    bytes::complete::{escaped_transform, is_not, tag, take, take_till},
    character::complete::{char, multispace1, satisfy},
    combinator::{all_consuming, map, not, opt, peek, recognize, value},
    error::{Error, ErrorKind},
    multi::{many0, many1, separated_list0, separated_list1},
    sequence::{delimited, preceded, terminated},
};
use std::error::Error as StdError;

/// A parse error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    message: String,
}

impl ParseError {
    fn from_nom<'input>(error: nom::Err<ParserError<'input>>) -> Self {
        let message = match error {
            nom::Err::Incomplete(_) => "incomplete input".to_string(),
            nom::Err::Error(error) | nom::Err::Failure(error) => error.to_string(),
        };

        Self { message }
    }
}

impl Display for ParseError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.message)
    }
}

impl StdError for ParseError {}

/// Parse a Relax NG compact syntax schema.
pub fn parse_schema(input: &str) -> Result<Schema, ParseError> {
    let mut parser = all_consuming(delimited(whitespace0, schema, whitespace0));

    match parser.parse(input) {
        Ok((_, schema)) => Ok(schema),
        Err(error) => Err(ParseError::from_nom(error)),
    }
}

type ParserError<'input> = Error<&'input str>;
type ParserResult<'input, Output> = IResult<&'input str, Output, ParserError<'input>>;

fn schema(input: &str) -> ParserResult<'_, Schema> {
    let (input, declarations) = many0(declaration).parse(input)?;
    let (input, _) = whitespace0(input)?;
    if input.is_empty() {
        return Ok((
            input,
            Schema {
                declarations,
                body: SchemaBody::Grammar(Grammar { items: Vec::new() }),
            },
        ));
    }

    let (input, body) = schema_body(input)?;

    Ok((input, Schema { declarations, body }))
}

fn schema_body(input: &str) -> ParserResult<'_, SchemaBody> {
    let (input, _) = skip_annotation_blocks(input)?;
    if grammar_item(input).is_ok() {
        let (remaining, items) = all_consuming(many1(grammar_item)).parse(input)?;
        return Ok((remaining, SchemaBody::Grammar(Grammar { items })));
    }

    let (input, pattern) = pattern(input)?;
    Ok((input, SchemaBody::Pattern(pattern)))
}

fn declaration(input: &str) -> ParserResult<'_, Declaration> {
    delimited(
        whitespace0,
        alt((
            map(namespace_declaration, Declaration::Namespace),
            map(default_namespace_declaration, Declaration::DefaultNamespace),
            map(datatypes_declaration, Declaration::Datatypes),
        )),
        whitespace0,
    )
    .parse(input)
}

fn namespace_declaration(input: &str) -> ParserResult<'_, NamespaceDeclaration> {
    let (input, _) = keyword("namespace").parse(input)?;
    let (input, prefix) = identifier_token(input)?;
    let (input, _) = symbol("=").parse(input)?;
    let (input, uri) = string_literal_token(input)?;

    Ok((input, NamespaceDeclaration { prefix, uri }))
}

fn default_namespace_declaration(input: &str) -> ParserResult<'_, String> {
    let (input, _) = keyword("default").parse(input)?;
    let (input, _) = keyword("namespace").parse(input)?;
    let (input, _) = opt(identifier_token).parse(input)?;
    let (input, _) = symbol("=").parse(input)?;
    let (input, uri) = string_literal_token(input)?;

    Ok((input, uri))
}

fn datatypes_declaration(input: &str) -> ParserResult<'_, DatatypesDeclaration> {
    let (input, _) = keyword("datatypes").parse(input)?;
    let (input, prefix) = opt(identifier_token).parse(input)?;
    let (input, _) = symbol("=").parse(input)?;
    let (input, uri) = string_literal_token(input)?;

    Ok((input, DatatypesDeclaration { prefix, uri }))
}

fn grammar(input: &str) -> ParserResult<'_, Grammar> {
    let mut remaining_input = input;
    let mut items = Vec::new();

    loop {
        let (after_annotations, _) = skip_annotation_blocks(remaining_input)?;
        if after_annotations != remaining_input {
            remaining_input = after_annotations;
        }

        match grammar_item(remaining_input) {
            Ok((next_input, item)) => {
                if next_input.len() == remaining_input.len() {
                    break;
                }
                items.push(item);
                remaining_input = next_input;
            }
            Err(_) => break,
        }
    }

    Ok((remaining_input, Grammar { items }))
}

fn grammar_item(input: &str) -> ParserResult<'_, GrammarItem> {
    let (input, item) = delimited(
        whitespace0,
        alt((
            start_item,
            map(annotation_element, GrammarItem::Annotation),
            define_item,
            div_item,
            include_item,
            map(namespace_declaration, GrammarItem::Namespace),
            map(default_namespace_declaration, GrammarItem::DefaultNamespace),
            map(datatypes_declaration, GrammarItem::Datatypes),
        )),
        whitespace0,
    )
    .parse(input)?;

    let (input, _) = many0(annotation_block_after).parse(input)?;

    Ok((input, item))
}

fn start_item(input: &str) -> ParserResult<'_, GrammarItem> {
    let (input, _) = keyword("start").parse(input)?;
    let (input, combine) = assignment_operator(input)?;
    let (input, pattern) = pattern(input)?;

    Ok((input, GrammarItem::Start { combine, pattern }))
}

fn define_item(input: &str) -> ParserResult<'_, GrammarItem> {
    let (input, name) = identifier_token(input)?;
    let (input, combine) = assignment_operator(input)?;
    let (input, pattern) = pattern(input)?;

    Ok((
        input,
        GrammarItem::Define(Definition {
            name,
            combine,
            pattern,
        }),
    ))
}

fn div_item(input: &str) -> ParserResult<'_, GrammarItem> {
    let (input, _) = keyword("div").parse(input)?;
    let (input, grammar) = delimited(symbol("{"), grammar, symbol("}")).parse(input)?;

    Ok((input, GrammarItem::Div(grammar)))
}

fn include_item(input: &str) -> ParserResult<'_, GrammarItem> {
    let (input, _) = keyword("include").parse(input)?;
    let (input, uri) = string_literal_token(input)?;
    let (input, inherit) = opt(inherit).parse(input)?;
    let (input, grammar) = opt(raw_grammar_block).parse(input)?;

    Ok((
        input,
        GrammarItem::Include(Include {
            uri,
            inherit,
            grammar,
        }),
    ))
}

fn raw_grammar_block(input: &str) -> ParserResult<'_, Grammar> {
    let (input, _) = symbol("{").parse(input)?;
    let mut depth = 1_u32;
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
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    let remaining_input = &input[offset_index + 1..];
                    return Ok((remaining_input, Grammar { items: Vec::new() }));
                }
            }
            _ => {}
        }
    }

    Err(nom::Err::Error(Error::new(input, ErrorKind::Tag)))
}

fn inherit(input: &str) -> ParserResult<'_, Inherit> {
    let (input, _) = keyword("inherit").parse(input)?;
    let (input, prefix) = opt(preceded(symbol("="), identifier_token)).parse(input)?;

    Ok((
        input,
        match prefix {
            Some(prefix) => Inherit::Prefix(prefix),
            None => Inherit::DefaultNamespace,
        },
    ))
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
    let (input, patterns) = separated_list1(symbol("|"), interleave_pattern).parse(input)?;
    Ok((input, fold_patterns(patterns, Pattern::Choice)))
}

fn interleave_pattern(input: &str) -> ParserResult<'_, Pattern> {
    let (input, patterns) = separated_list1(symbol("&"), group_pattern).parse(input)?;
    Ok((input, fold_patterns(patterns, Pattern::Interleave)))
}

fn group_pattern(input: &str) -> ParserResult<'_, Pattern> {
    let (input, patterns) = separated_list1(symbol(","), quantified_pattern).parse(input)?;
    Ok((input, fold_patterns(patterns, Pattern::Group)))
}

fn quantified_pattern(input: &str) -> ParserResult<'_, Pattern> {
    let (input, base_pattern) = primary_pattern(input)?;
    let (input, _) = many0(annotation_attachment).parse(input)?;
    let (input, quantifier) = opt(alt((
        value("?", symbol("?")),
        value("*", symbol("*")),
        value("+", symbol("+")),
    )))
    .parse(input)?;

    let pattern = match quantifier {
        Some("?") => Pattern::Optional(Box::new(base_pattern)),
        Some("*") => Pattern::ZeroOrMore(Box::new(base_pattern)),
        Some("+") => Pattern::OneOrMore(Box::new(base_pattern)),
        None => base_pattern,
        Some(_) => base_pattern,
    };

    Ok((input, pattern))
}

fn annotation_attachment(input: &str) -> ParserResult<'_, ()> {
    let (input, _) = symbol(">>").parse(input)?;
    let (input, _) = annotation_element(input)?;
    Ok((input, ()))
}

fn primary_pattern(input: &str) -> ParserResult<'_, Pattern> {
    let (input, _) = skip_annotation_blocks(input)?;
    alt((
        element_pattern,
        attribute_pattern,
        list_pattern,
        mixed_pattern,
        grammar_pattern,
        parent_pattern,
        external_pattern,
        text_pattern,
        empty_pattern,
        not_allowed_pattern,
        delimited(symbol("("), pattern, symbol(")")),
        value_pattern,
        data_pattern,
        name_pattern,
    ))
    .parse(input)
}

fn element_pattern(input: &str) -> ParserResult<'_, Pattern> {
    let (input, _) = keyword("element").parse(input)?;
    let (input, name_class) = name_class(input)?;
    let (input, pattern) = delimited(symbol("{"), pattern, symbol("}")).parse(input)?;

    Ok((
        input,
        Pattern::Element {
            name_class,
            pattern: Box::new(pattern),
        },
    ))
}

fn attribute_pattern(input: &str) -> ParserResult<'_, Pattern> {
    let (input, _) = keyword("attribute").parse(input)?;
    let (input, name_class) = name_class(input)?;
    let (input, pattern) = delimited(symbol("{"), pattern, symbol("}")).parse(input)?;

    Ok((
        input,
        Pattern::Attribute {
            name_class,
            pattern: Box::new(pattern),
        },
    ))
}

fn list_pattern(input: &str) -> ParserResult<'_, Pattern> {
    let (input, _) = keyword("list").parse(input)?;
    let (input, pattern) = delimited(symbol("{"), pattern, symbol("}")).parse(input)?;
    Ok((input, Pattern::List(Box::new(pattern))))
}

fn mixed_pattern(input: &str) -> ParserResult<'_, Pattern> {
    let (input, _) = keyword("mixed").parse(input)?;
    let (input, pattern) = delimited(symbol("{"), pattern, symbol("}")).parse(input)?;
    Ok((input, Pattern::Mixed(Box::new(pattern))))
}

fn grammar_pattern(input: &str) -> ParserResult<'_, Pattern> {
    let (input, _) = keyword("grammar").parse(input)?;
    let (input, grammar) = delimited(symbol("{"), grammar, symbol("}")).parse(input)?;
    Ok((input, Pattern::Grammar(grammar)))
}

fn parent_pattern(input: &str) -> ParserResult<'_, Pattern> {
    let (input, _) = keyword("parent").parse(input)?;
    let (input, name) = identifier_token(input)?;
    Ok((input, Pattern::ParentRef(name)))
}

fn external_pattern(input: &str) -> ParserResult<'_, Pattern> {
    let (input, _) = keyword("external").parse(input)?;
    let (input, uri) = string_literal_token(input)?;
    Ok((input, Pattern::ExternalRef(uri)))
}

fn text_pattern(input: &str) -> ParserResult<'_, Pattern> {
    let (input, _) = keyword("text").parse(input)?;
    Ok((input, Pattern::Text))
}

fn empty_pattern(input: &str) -> ParserResult<'_, Pattern> {
    let (input, _) = keyword("empty").parse(input)?;
    Ok((input, Pattern::Empty))
}

fn not_allowed_pattern(input: &str) -> ParserResult<'_, Pattern> {
    let (input, _) = keyword("notAllowed").parse(input)?;
    Ok((input, Pattern::NotAllowed))
}

fn name_pattern(input: &str) -> ParserResult<'_, Pattern> {
    let (input, name) = name_token(input)?;
    Ok((input, Pattern::Name(name)))
}

fn data_pattern(input: &str) -> ParserResult<'_, Pattern> {
    let starting_input = input;
    let (input, name) = name_token(input)?;
    let (input, parameters) = opt(parameters).parse(input)?;
    let (input, except_pattern) = opt(preceded(symbol("-"), pattern)).parse(input)?;

    if parameters.is_none() && except_pattern.is_none() {
        return Err(nom::Err::Error(Error::new(starting_input, ErrorKind::Tag)));
    }

    Ok((
        input,
        Pattern::Data {
            name,
            parameters: parameters.unwrap_or_default(),
            except: except_pattern.map(Box::new),
        },
    ))
}

fn value_pattern(input: &str) -> ParserResult<'_, Pattern> {
    let (input, datatype_name) = opt(terminated(name_token_leading, whitespace1)).parse(input)?;
    let (input, value) = string_literal_token(input)?;

    Ok((
        input,
        Pattern::Value {
            name: datatype_name,
            value,
        },
    ))
}

fn name_class(input: &str) -> ParserResult<'_, NameClass> {
    name_class_choice(input)
}

fn name_class_choice(input: &str) -> ParserResult<'_, NameClass> {
    let (input, classes) = separated_list1(symbol("|"), name_class_except).parse(input)?;
    Ok((input, fold_name_classes(classes)))
}

fn name_class_except(input: &str) -> ParserResult<'_, NameClass> {
    let (input, base) = name_class_primary(input)?;
    let (input, except) = opt(preceded(symbol("-"), name_class_primary)).parse(input)?;

    match except {
        Some(except) => Ok((
            input,
            NameClass::Except {
                base: Box::new(base),
                except: Box::new(except),
            },
        )),
        None => Ok((input, base)),
    }
}

fn name_class_primary(input: &str) -> ParserResult<'_, NameClass> {
    delimited(
        whitespace0,
        alt((
            value(NameClass::AnyName, tag("*")),
            map((identifier, char(':'), char('*')), |(prefix, _, _)| {
                NameClass::NsName(Some(prefix))
            }),
            map(name, NameClass::Name),
            delimited(tag("("), name_class, tag(")")),
        )),
        whitespace0,
    )
    .parse(input)
}

fn parameters(input: &str) -> ParserResult<'_, Vec<Parameter>> {
    delimited(
        symbol("{"),
        separated_list0(parameter_separator, parameter),
        symbol("}"),
    )
    .parse(input)
}

fn parameter_separator(input: &str) -> ParserResult<'_, ()> {
    alt((value((), symbol(",")), whitespace1)).parse(input)
}

fn parameter(input: &str) -> ParserResult<'_, Parameter> {
    let (input, name) = name_token(input)?;
    let (input, _) = symbol("=").parse(input)?;
    let (input, value) = string_literal(input)?;

    Ok((input, Parameter { name, value }))
}

fn annotation_element(input: &str) -> ParserResult<'_, Annotation> {
    let (input, name) = name_token(input)?;
    let (input, attributes) = annotation_block(input)?;

    Ok((input, Annotation { name, attributes }))
}

fn annotation_block(input: &str) -> ParserResult<'_, Vec<AnnotationAttribute>> {
    alt((annotation_block_attributes, annotation_block_raw)).parse(input)
}

fn annotation_block_attributes(input: &str) -> ParserResult<'_, Vec<AnnotationAttribute>> {
    let (input, _) = open_bracket(input)?;
    let (input, attributes) =
        separated_list0(annotation_separator, annotation_attribute).parse(input)?;
    let (input, _) = whitespace0(input)?;
    let (input, _) = close_bracket(input)?;

    Ok((input, attributes))
}

fn annotation_block_raw(input: &str) -> ParserResult<'_, Vec<AnnotationAttribute>> {
    let (input, _) = open_bracket(input)?;
    let mut depth = 1_u32;
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
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    let remaining_input = &input[offset_index + 1..];
                    return Ok((remaining_input, Vec::new()));
                }
            }
            _ => {}
        }
    }

    Err(nom::Err::Error(Error::new(input, ErrorKind::Tag)))
}

fn annotation_separator(input: &str) -> ParserResult<'_, ()> {
    let (input, _) = peek(preceded(whitespace1, annotation_attribute)).parse(input)?;
    let (input, _) = whitespace1(input)?;
    Ok((input, ()))
}

fn annotation_block_after(input: &str) -> ParserResult<'_, Vec<AnnotationAttribute>> {
    let (input_after_space, _) = whitespace0(input)?;
    if peek(open_bracket).parse(input_after_space).is_err() {
        return Err(nom::Err::Error(Error::new(input, ErrorKind::Tag)));
    }

    annotation_block(input_after_space)
}

fn skip_annotation_blocks(input: &str) -> ParserResult<'_, ()> {
    let (input, _) = many0(annotation_block_after).parse(input)?;
    Ok((input, ()))
}

fn open_bracket(input: &str) -> ParserResult<'_, &str> {
    tag("[").parse(input)
}

fn close_bracket(input: &str) -> ParserResult<'_, &str> {
    tag("]").parse(input)
}

fn annotation_attribute(input: &str) -> ParserResult<'_, AnnotationAttribute> {
    let (input, name) = name_token(input)?;
    let (input, _) = symbol("=").parse(input)?;
    let (input, value) = string_literal(input)?;

    Ok((input, AnnotationAttribute { name, value }))
}

fn identifier_token(input: &str) -> ParserResult<'_, String> {
    spaced(identifier).parse(input)
}

fn name_token(input: &str) -> ParserResult<'_, Name> {
    spaced(name).parse(input)
}

fn name_token_leading(input: &str) -> ParserResult<'_, Name> {
    preceded(whitespace0, name).parse(input)
}

fn string_literal_token(input: &str) -> ParserResult<'_, String> {
    spaced(string_literal).parse(input)
}

fn spaced<'input, Output, ParserType>(
    parser: ParserType,
) -> impl Parser<&'input str, Output = Output, Error = ParserError<'input>>
where
    ParserType: Parser<&'input str, Output = Output, Error = ParserError<'input>>,
{
    delimited(whitespace0, parser, whitespace0)
}

fn keyword(keyword_text: &'static str) -> impl FnMut(&str) -> ParserResult<'_, &str> {
    move |input| {
        delimited(
            whitespace0,
            terminated(tag(keyword_text), not(peek(satisfy(is_identifier_char)))),
            whitespace0,
        )
        .parse(input)
    }
}

fn symbol(symbol_text: &'static str) -> impl FnMut(&str) -> ParserResult<'_, &str> {
    move |input| delimited(whitespace0, tag(symbol_text), whitespace0).parse(input)
}

fn whitespace0(input: &str) -> ParserResult<'_, ()> {
    let (input, _) = many0(alt((value((), multispace1), comment))).parse(input)?;
    Ok((input, ()))
}

fn whitespace1(input: &str) -> ParserResult<'_, ()> {
    let (input, _) = many1(alt((value((), multispace1), comment))).parse(input)?;
    Ok((input, ()))
}

fn comment(input: &str) -> ParserResult<'_, ()> {
    let (input, _) = preceded(tag("#"), take_till(|character| character == '\n')).parse(input)?;
    let (input, _) = opt(char('\n')).parse(input)?;
    Ok((input, ()))
}

fn name(input: &str) -> ParserResult<'_, Name> {
    let (input, first) = identifier(input)?;
    let (input, rest) = opt(preceded(char(':'), identifier)).parse(input)?;

    let (prefix, local) = match rest {
        Some(local) => (Some(first), local),
        None => (None, first),
    };

    Ok((input, Name { prefix, local }))
}

fn identifier(input: &str) -> ParserResult<'_, String> {
    let (input, _) = opt(char('\\')).parse(input)?;
    let (input, value) = recognize((
        satisfy(is_identifier_start),
        many0(satisfy(is_identifier_char)),
    ))
    .parse(input)?;

    Ok((input, value.to_string()))
}

fn is_identifier_start(character: char) -> bool {
    character.is_ascii_alphabetic() || character == '_'
}

fn is_identifier_char(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == '_' || character == '-' || character == '.'
}

fn string_literal(input: &str) -> ParserResult<'_, String> {
    let (input, delimiter) = alt((char('"'), char('\''))).parse(input)?;
    let (input, value) = if delimiter == '"' {
        let (input, value) =
            opt(escaped_transform(is_not("\\\""), '\\', string_escape)).parse(input)?;
        (input, value.unwrap_or_default())
    } else {
        let (input, value) =
            opt(escaped_transform(is_not("\\'"), '\\', string_escape)).parse(input)?;
        (input, value.unwrap_or_default())
    };
    let (input, _) = char(delimiter).parse(input)?;

    Ok((input, value))
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

fn fold_name_classes(classes: Vec<NameClass>) -> NameClass {
    if classes.len() == 1 {
        classes
            .into_iter()
            .next()
            .expect("name class list must contain one item")
    } else {
        NameClass::Choice(classes)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        ast::{
            Annotation, AnnotationAttribute, Combine, Declaration, Definition, Grammar,
            GrammarItem, Name, NameClass, NamespaceDeclaration, Parameter, Pattern, Schema,
            SchemaBody,
        },
        parse_schema,
    };

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
                    Pattern::OneOrMore(Box::new(Pattern::Choice(vec![
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
        let input = r#"
namespace sch = "http://example.com/sch"

sch:ns [ prefix = "html" uri = "http://example.com/ns" ]
start = element html { empty }
common &= element div { text }
"#;

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
                    GrammarItem::Define(Definition {
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

        let annotation_result = super::annotation_element(input);
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
        let input = "xml.space.attrib = # added -- hsivonen\n    attribute xml:space { string \"preserve\" }?";

        let result = super::grammar_item(input);

        assert!(result.is_ok(), "inline comment parse failed: {result:?}");
    }

    #[test]
    fn parse_definition_followed_by_next_item() {
        let input = "xml.space.attrib = # added -- hsivonen\n    attribute xml:space { string \"preserve\" }?\nclass.attrib = attribute class { text }?";

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
        let input = "attribute xml:space { string \"preserve\" }?\nclass.attrib = attribute class { text }?";

        let (remaining_input, _) = super::pattern(input).expect("attribute pattern should parse");

        assert!(
            remaining_input.trim_start().starts_with("class.attrib"),
            "expected to stop at next definition, got: {remaining_input:?}"
        );
    }

    #[test]
    fn parse_attribute_pattern_parser() {
        let input = "attribute xml:space { string \"preserve\" }?\nclass.attrib = attribute class { text }?";

        let (remaining_input, _) = super::attribute_pattern(input).expect("attribute should parse");

        assert!(
            remaining_input.trim_start().starts_with("?"),
            "expected quantifier, got: {remaining_input:?}"
        );
    }

    #[test]
    fn parse_default_namespace_with_prefix() {
        let input = "default namespace svg = \"http://example.com\"\n\nelement svg { empty }";

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
        let input =
            "include \"base.rnc\" {\n    [ sch:pattern [ name = \"x\" ] ]\n    start = empty\n}";

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
        let input = "include \"basic-form.rnc\" {\n    [\n        sch:pattern [\n            name = \"select.multiple\"\n            \"\\x{a}\"\n        ]\n    ]\n    select = element select { select.attlist, (option | optgroup)+ }\n}";

        let result = parse_schema(input);

        assert!(
            result.is_ok(),
            "nested annotation include failed: {result:?}"
        );
    }

    #[test]
    fn parse_annotation_attachment_with_comment() {
        let input = "element button {\n    button.attlist,\n    Flow.model\n    # comment\n    >> sch:pattern [ name = \"button.content\" ]\n}";

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
        let input = "include \"basic-form.rnc\" {\n    [\n        sch:pattern [\n            name = \"select.multiple\"\n            sch:report [\n                test = \"html:option[@selected]\"\n            ]\n        ]\n    ]\n    select = element select { select.attlist, (option | optgroup)+ }\n}";

        let result = parse_schema(input);

        assert!(
            result.is_ok(),
            "bracketed annotation include failed: {result:?}"
        );
    }

    #[test]
    fn parse_include_with_schematron_block() {
        let input = "include \"basic-form.rnc\" {\n    [\n        sch:pattern [\n            name = \"select.multiple.selected.options\"\n            sch:report [\n                test = \"not(@multiple) and count(html:option[@selected]) > 1\"\n            ]\n        ]\n    ]\n    select = element select { select.attlist, (option | optgroup)+ }\n}";

        let result = parse_schema(input);

        assert!(
            result.is_ok(),
            "schematron include parse failed: {result:?}"
        );
    }

    #[test]
    fn parse_include_block_as_grammar_item() {
        let input = "include \"basic-form.rnc\" {\n    [\n        sch:pattern [\n            name = \"select.multiple.selected.options\"\n            sch:report [\n                test = \"not(@multiple) and count(html:option[@selected]) > 1\"\n            ]\n        ]\n    ]\n    select = element select { select.attlist, (option | optgroup)+ }\n}\nform.attlist &= attribute accept-charset { charsets.datatype }?";

        let (remaining_input, _) = super::grammar_item(input).expect("include item should parse");

        assert!(
            remaining_input.trim_start().starts_with("form.attlist"),
            "expected to stop at form.attlist, got: {remaining_input:?}"
        );
    }

    #[test]
    fn parse_raw_include_block() {
        let input = "{\n    [\n        sch:pattern [\n            name = \"select.multiple.selected.options\"\n            sch:report [\n                test = \"not(@multiple) and count(html:option[@selected]) > 1\"\n            ]\n        ]\n    ]\n    select = element select { select.attlist, (option | optgroup)+ }\n}\nform.attlist &= attribute accept-charset { charsets.datatype }?";

        let (remaining_input, _) = super::raw_grammar_block(input).expect("raw block should parse");

        assert!(
            remaining_input.trim_start().starts_with("form.attlist"),
            "expected to stop at form.attlist, got: {remaining_input:?}"
        );
    }

    #[test]
    fn parse_annotation_before_include_block() {
        let input = "sch:ns [ prefix = \"html\" uri = \"http://www.w3.org/1999/xhtml\" ]\ninclude \"basic-form.rnc\" {\n    [\n        sch:pattern [\n            name = \"select.multiple.selected.options\"\n            sch:report [\n                test = \"not(@multiple) and count(html:option[@selected]) > 1\"\n            ]\n        ]\n    ]\n    select = element select { select.attlist, (option | optgroup)+ }\n}\nform.attlist &= attribute accept-charset { charsets.datatype }?";

        let result = parse_schema(input);

        assert!(
            result.is_ok(),
            "annotation include parse failed: {result:?}"
        );
    }

    #[test]
    fn parse_raw_include_block_with_escape_sequences() {
        let input = "{\n    [\n        sch:pattern [\n            name = \"select.multiple.selected.options\"\n            \"\\x{a}\" ~\n            \"          \"\n            sch:rule [\n                context = \"html:select\"\n                \"\\x{a}\" ~\n                \"              \"\n                sch:report [\n                    test =\n                        \"not(@multiple) and count(html:option[@selected]) > 1\"\n                    \"\\x{a}\" ~\n                    \"                   Select elements which aren't marked as multiple may not have more then one selected option.\\x{a}\" ~\n                    \"              \"\n                ]\n                \"\\x{a}\" ~\n                \"          \"\n            ]\n            \"\\x{a}\" ~\n            \"      \"\n        ]\n    ]\n    select = element select { select.attlist, (option | optgroup)+ }\n}\nform.attlist &= attribute accept-charset { charsets.datatype }?";

        let (remaining_input, _) =
            super::raw_grammar_block(input).expect("raw include should parse");

        assert!(
            remaining_input.trim_start().starts_with("form.attlist"),
            "expected to stop at form.attlist, got: {remaining_input:?}"
        );
    }

    #[test]
    fn parse_choice_with_inline_comment() {
        let input = "InputType.class |=\n    string \"image\"\n    | string \"button\"\n    | # bugfix\n      string \"file\"";

        let result = super::grammar_item(input);

        assert!(result.is_ok(), "choice with comment failed: {result:?}");
    }
}
