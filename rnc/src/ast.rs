//! Relax NG Compact Syntax abstract syntax tree types.

/// A Relax NG compact syntax schema.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Schema {
    /// Declarations appearing before the schema body.
    pub declarations: Vec<Declaration>,
    /// The schema body, either a pattern or an implicit grammar.
    pub body: SchemaBody,
}

/// A schema body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchemaBody {
    /// A top-level pattern.
    Pattern(Pattern),
    /// An implicit grammar with grammar items.
    Grammar(Grammar),
}

/// A grammar block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Grammar {
    /// Grammar items contained in the block.
    pub items: Vec<GrammarItem>,
}

/// A top-level declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Declaration {
    /// A namespace declaration.
    Namespace(NamespaceDeclaration),
    /// A default namespace declaration.
    DefaultNamespace(String),
    /// A datatype library declaration.
    Datatypes(DatatypesDeclaration),
}

/// A namespace declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamespaceDeclaration {
    /// The namespace prefix.
    pub prefix: String,
    /// The namespace URI.
    pub uri: String,
}

/// A datatype library declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatatypesDeclaration {
    /// The datatype prefix, if any.
    pub prefix: Option<String>,
    /// The datatype library URI.
    pub uri: String,
}

/// Grammar items that appear in a grammar block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GrammarItem {
    /// A start pattern.
    Start {
        /// The combine operator, if any.
        combine: Option<Combine>,
        /// The start pattern.
        pattern: Pattern,
    },
    /// A definition.
    Define(Definition),
    /// A div block containing nested grammar items.
    Div(Grammar),
    /// An include block.
    Include(Include),
    /// An annotation element.
    Annotation(Annotation),
    /// A namespace declaration inside a grammar.
    Namespace(NamespaceDeclaration),
    /// A default namespace declaration inside a grammar.
    DefaultNamespace(String),
    /// A datatype library declaration inside a grammar.
    Datatypes(DatatypesDeclaration),
}

/// A definition item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Definition {
    /// The definition name.
    pub name: String,
    /// The combine operator, if any.
    pub combine: Option<Combine>,
    /// The pattern for this definition.
    pub pattern: Pattern,
}

/// A grammar include item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Include {
    /// The included schema URI.
    pub uri: String,
    /// The inherit modifier, if any.
    pub inherit: Option<Inherit>,
    /// The included grammar items.
    pub grammar: Option<Grammar>,
}

/// An inherit modifier for an include item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Inherit {
    /// Inherit the default namespace.
    DefaultNamespace,
    /// Inherit a named namespace prefix.
    Prefix(String),
}

/// A combine operator for definitions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Combine {
    /// A choice combine operator (`|=`).
    Choice,
    /// An interleave combine operator (`&=`).
    Interleave,
}

/// A schema pattern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pattern {
    /// A choice pattern (`|`).
    Choice(Vec<Self>),
    /// An interleave pattern (`&`).
    Interleave(Vec<Self>),
    /// A group pattern (`,`) representing sequence.
    Group(Vec<Self>),
    /// An optional pattern (`?`).
    Optional(Box<Self>),
    /// A zero-or-more pattern (`*`).
    ZeroOrMore(Box<Self>),
    /// A one-or-more pattern (`+`).
    OneOrMore(Box<Self>),
    /// A list pattern.
    List(Box<Self>),
    /// A mixed pattern.
    Mixed(Box<Self>),
    /// An element pattern.
    Element {
        /// The element name class.
        name_class: NameClass,
        /// The element content pattern.
        pattern: Box<Self>,
    },
    /// An attribute pattern.
    Attribute {
        /// The attribute name class.
        name_class: NameClass,
        /// The attribute value pattern.
        pattern: Box<Self>,
    },
    /// A data pattern.
    Data {
        /// The datatype name.
        name: Name,
        /// The datatype parameters.
        parameters: Vec<Parameter>,
        /// An optional except pattern.
        except: Option<Box<Self>>,
    },
    /// A value pattern.
    Value {
        /// The datatype name, if specified.
        name: Option<Name>,
        /// The literal value.
        value: String,
    },
    /// A text pattern.
    Text,
    /// An empty pattern.
    Empty,
    /// A not-allowed pattern.
    NotAllowed,
    /// A named pattern that requires later semantic resolution.
    Name(Name),
    /// A reference to a parent grammar definition.
    ParentRef(String),
    /// An external reference.
    ExternalRef(String),
    /// A nested grammar pattern.
    Grammar(Grammar),
}

/// A name used in a Relax NG schema.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Name {
    /// The optional namespace prefix.
    pub prefix: Option<String>,
    /// The local name.
    pub local: String,
}

/// A name class for element and attribute patterns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NameClass {
    /// A single name.
    Name(Name),
    /// A namespace wildcard for a prefix.
    NsName(Option<String>),
    /// Any name.
    AnyName,
    /// A choice of name classes.
    Choice(Vec<Self>),
    /// A name class with exclusions.
    Except {
        /// The base name class.
        base: Box<Self>,
        /// The excluded name class.
        except: Box<Self>,
    },
}

/// A datatype parameter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parameter {
    /// The parameter name.
    pub name: Name,
    /// The parameter value.
    pub value: String,
}

/// An annotation element.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Annotation {
    /// The annotation element name.
    pub name: Name,
    /// The annotation attributes.
    pub attributes: Vec<AnnotationAttribute>,
}

/// An annotation attribute.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnnotationAttribute {
    /// The attribute name.
    pub name: Name,
    /// The attribute value.
    pub value: String,
}
