/// A schema.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Schema {
    /// Declarations.
    pub declarations: Vec<Declaration>,
    /// A body.
    pub body: SchemaBody,
}

/// A schema body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchemaBody {
    /// A grammar.
    Grammar(Grammar),
    /// A pattern.
    Pattern(Pattern),
}

/// A grammar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Grammar {
    /// Contents.
    pub contents: Vec<GrammarContent>,
}

/// A declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Declaration {
    /// A datatype library declaration.
    Datatypes(DatatypesDeclaration),
    /// A default namespace declaration.
    DefaultNamespace(String),
    /// A namespace declaration.
    Namespace(NamespaceDeclaration),
}

/// A namespace declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamespaceDeclaration {
    /// A prefix.
    pub prefix: Identifier,
    /// A URI.
    pub uri: String,
}

/// A datatype library declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatatypesDeclaration {
    /// A prefix.
    pub prefix: Option<Identifier>,
    /// A URI.
    pub uri: String,
}

/// A grammar content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GrammarContent {
    /// An annotation.
    Annotation(AnnotationElement),
    /// A definition.
    Definition(Definition),
    /// A div block containing nested grammar contents.
    Div(Grammar),
    /// An include block.
    Include(Include),
    /// A start.
    Start {
        /// A combine operator.
        combine: Option<Combine>,
        /// A pattern.
        pattern: Pattern,
    },
}

// TODO Add `LocalGrammarContent`.

/// A definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Definition {
    /// A name.
    pub name: Identifier,
    /// A combine operator.
    pub combine: Option<Combine>,
    /// A pattern.
    pub pattern: Pattern,
}

/// An include.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Include {
    /// A URI.
    pub uri: String,
    /// An inherit modifier.
    pub inherit: Option<Inherit>,
    /// A grammar.
    // TODO Use `LocalGrammar`.
    pub grammar: Option<Grammar>,
}

// TODO Make this `struct`.
/// An inherit modifier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Inherit {
    /// Inherits a namespace.
    Prefix(Identifier),
}

/// A combine operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Combine {
    /// A choice operator.
    Choice,
    /// An interleave operator.
    Interleave,
}

/// A schema pattern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pattern {
    /// An attribute.
    Attribute {
        /// A name class.
        name_class: NameClass,
        /// A pattern.
        pattern: Box<Self>,
    },
    /// A choice pattern.
    Choice(Vec<Self>),
    /// Data.
    Data {
        /// A name.
        name: Name,
        /// Parameters.
        parameters: Vec<Parameter>,
        /// An except pattern.
        except: Option<Box<Self>>,
    },
    /// An element.
    Element {
        /// A name class.
        name_class: NameClass,
        /// A pattern.
        pattern: Box<Self>,
    },
    /// An empty pattern.
    Empty,
    /// An external reference.
    External(String),
    /// A grammar.
    Grammar(Grammar),
    /// A group.
    Group(Vec<Self>),
    /// An interleave pattern.
    Interleave(Vec<Self>),
    /// A list pattern.
    List(Box<Self>),
    /// A repetition more than or equal to 0.
    Many0(Box<Self>),
    /// A repetition more than 0.
    Many1(Box<Self>),
    /// A name.
    Name(Name),
    /// A not-allowed pattern.
    NotAllowed,
    /// An optional pattern.
    Optional(Box<Self>),
    /// A text.
    Text,
    /// A value.
    Value {
        /// A name.
        name: Option<Name>,
        /// A value.
        value: String,
    },
}

/// A name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Name {
    /// A prefix.
    pub prefix: Option<Identifier>,
    /// A local name.
    pub local: Identifier,
}

/// A name class.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NameClass {
    /// Any name.
    AnyName,
    /// A choice of name classes.
    Choice(Vec<Self>),
    /// A name class with exclusions.
    Except {
        /// A base name class.
        base: Box<Self>,
        /// The excluded name class.
        except: Box<Self>,
    },
    /// A name.
    Name(Name),
    /// A namespace wildcard for a prefix.
    NamespaceName(Option<Identifier>),
}

/// A datatype parameter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parameter {
    /// A name.
    pub name: Name,
    /// A value.
    pub value: String,
}

/// An annotation element.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnnotationElement {
    /// A name.
    pub name: Name,
    /// Attributes.
    pub attributes: Vec<AnnotationAttribute>,
}

/// An annotation attribute.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnnotationAttribute {
    /// A name.
    pub name: Name,
    /// A value.
    pub value: String,
}

/// An identifier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identifier {
    /// A component.
    pub component: String,
    /// Sub-components.
    pub sub_components: Vec<String>,
}
