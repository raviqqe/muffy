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
    /// Items.
    pub items: Vec<GrammarItem>,
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
    pub prefix: String,
    /// A URI.
    pub uri: String,
}

/// A datatype library declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatatypesDeclaration {
    /// A prefix.
    pub prefix: Option<String>,
    /// A URI.
    pub uri: String,
}

/// A grammar item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GrammarItem {
    /// An annotation.
    Annotation(Annotation),
    /// A datatype library declaration.
    Datatypes(DatatypesDeclaration),
    /// A default namespace declaration.
    DefaultNamespace(String),
    /// A definition.
    Definition(Definition),
    /// A div block containing nested grammar items.
    Div(Grammar),
    /// An include block.
    Include(Include),
    /// A namespace declaration.
    Namespace(NamespaceDeclaration),
    /// A start.
    Start {
        /// A combine operator.
        combine: Option<Combine>,
        /// A pattern.
        pattern: Pattern,
    },
}

/// A definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Definition {
    /// A name.
    pub name: String,
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
    pub grammar: Option<Grammar>,
}

/// An inherit modifier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Inherit {
    /// Inherits the default namespace.
    DefaultNamespace,
    /// Inherits a namespace.
    Prefix(String),
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
    ExternalRef(String),
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
    /// A mixed pattern.
    Mixed(Box<Self>),
    /// A name.
    Name(Name),
    /// A not-allowed pattern.
    NotAllowed,
    /// An optional pattern.
    Optional(Box<Self>),
    /// A reference to a parent grammar definition.
    ParentRef(String),
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
    pub prefix: Option<String>,
    /// A local name.
    pub local: String,
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
    NamespaceName(Option<String>),
}

/// A datatype parameter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parameter {
    /// The parameter name.
    pub name: Name,
    /// The parameter value.
    pub value: String,
}

/// An annotation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Annotation {
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
