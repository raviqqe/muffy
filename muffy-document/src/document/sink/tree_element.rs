use markup5ever::{Attribute, QualName};

pub struct TreeElement {
    pub name: QualName,
    pub attributes: Vec<Attribute>,
    pub template_contents: Option<usize>,
    pub mathml_annotation_xml_integration_point: bool,
}
