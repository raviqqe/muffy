use super::tree_node::TreeNode;
use core::cell::RefCell;
use markup5ever::{Attribute, QualName};

pub struct TreeElement<'a> {
    pub name: QualName,
    pub attributes: RefCell<Vec<Attribute>>,
    pub template_contents: Option<&'a TreeNode<'a>>,
    pub mathml_annotation_xml_integration_point: bool,
}
