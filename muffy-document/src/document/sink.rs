mod tree_element;
mod tree_node;
mod tree_node_data;

use self::{tree_element::TreeElement, tree_node::TreeNode, tree_node_data::TreeNodeData};
use super::Document;
use alloc::borrow::Cow;
use core::{cell::RefCell, ptr};
use markup5ever::{
    Attribute, QualName,
    interface::{ElementFlags, NodeOrText, QuirksMode, TreeSink},
    tendril::StrTendril,
};
use typed_arena::Arena;

pub(crate) struct DocumentSink<'a> {
    arena: &'a Arena<TreeNode<'a>>,
    document: &'a TreeNode<'a>,
    errors: RefCell<Vec<Cow<'static, str>>>,
}

impl<'a> DocumentSink<'a> {
    pub fn new(arena: &'a Arena<TreeNode<'a>>) -> Self {
        Self {
            arena,
            document: arena.alloc(TreeNode::new(TreeNodeData::Document)),
            errors: Default::default(),
        }
    }

    fn create(&self, data: TreeNodeData<'a>) -> &'a TreeNode<'a> {
        self.arena.alloc(TreeNode::new(data))
    }

    fn insert(
        &self,
        previous: Option<&'a TreeNode<'a>>,
        child: NodeOrText<&'a TreeNode<'a>>,
        attach: impl FnOnce(&'a TreeNode<'a>),
    ) {
        match child {
            NodeOrText::AppendNode(node) => attach(node),
            NodeOrText::AppendText(text) => {
                if let Some(TreeNodeData::Text(previous)) = previous.map(|node| &node.data) {
                    previous.borrow_mut().push_tendril(&text);
                } else {
                    attach(self.create(TreeNodeData::Text(text.into())));
                }
            }
        }
    }
}

impl<'a> TreeSink for DocumentSink<'a> {
    type Handle = &'a TreeNode<'a>;
    type Output = (Document, Vec<Cow<'static, str>>);
    type ElemName<'b>
        = &'b QualName
    where
        Self: 'b;

    fn finish(self) -> Self::Output {
        (
            Document::new(self.document.build_children()),
            self.errors.into_inner(),
        )
    }

    fn parse_error(&self, message: Cow<'static, str>) {
        self.errors.borrow_mut().push(message);
    }

    fn get_document(&self) -> Self::Handle {
        self.document
    }

    fn elem_name(&self, target: &Self::Handle) -> Self::ElemName<'_> {
        &target.element().name
    }

    fn create_element(
        &self,
        name: QualName,
        attributes: Vec<Attribute>,
        flags: ElementFlags,
    ) -> Self::Handle {
        self.create(TreeNodeData::Element(TreeElement {
            name,
            attributes: attributes.into(),
            template_contents: flags.template.then(|| self.create(TreeNodeData::Document)),
            mathml_annotation_xml_integration_point: flags.mathml_annotation_xml_integration_point,
        }))
    }

    fn create_comment(&self, _text: StrTendril) -> Self::Handle {
        self.create(TreeNodeData::Comment)
    }

    fn create_pi(&self, _target: StrTendril, _data: StrTendril) -> Self::Handle {
        self.create(TreeNodeData::ProcessingInstruction)
    }

    fn append(&self, parent: &Self::Handle, child: NodeOrText<Self::Handle>) {
        self.insert(parent.last_child(), child, |node| parent.append(node));
    }

    fn append_based_on_parent_node(
        &self,
        element: &Self::Handle,
        previous_element: &Self::Handle,
        child: NodeOrText<Self::Handle>,
    ) {
        if element.parent().is_some() {
            self.append_before_sibling(element, child);
        } else {
            self.append(previous_element, child);
        }
    }

    fn append_doctype_to_document(
        &self,
        _name: StrTendril,
        _public_id: StrTendril,
        _system_id: StrTendril,
    ) {
        self.document.append(self.create(TreeNodeData::Doctype));
    }

    fn get_template_contents(&self, target: &Self::Handle) -> Self::Handle {
        target
            .element()
            .template_contents
            .expect("template contents")
    }

    fn same_node(&self, one: &Self::Handle, other: &Self::Handle) -> bool {
        ptr::eq(*one, *other)
    }

    fn set_quirks_mode(&self, _mode: QuirksMode) {}

    fn append_before_sibling(&self, sibling: &Self::Handle, child: NodeOrText<Self::Handle>) {
        self.insert(sibling.previous_sibling(), child, |node| {
            node.insert_before(sibling)
        });
    }

    fn add_attrs_if_missing(&self, target: &Self::Handle, mut attributes: Vec<Attribute>) {
        let mut existing_attributes = target.element().attributes.borrow_mut();

        attributes.retain(|attribute| {
            existing_attributes
                .iter()
                .all(|other| other.name != attribute.name)
        });
        existing_attributes.extend(attributes);
    }

    fn remove_from_parent(&self, target: &Self::Handle) {
        target.detach();
    }

    // spell-checker: disable-next-line
    fn reparent_children(&self, node: &Self::Handle, new_parent: &Self::Handle) {
        node.move_children(new_parent);
    }

    fn is_mathml_annotation_xml_integration_point(&self, handle: &Self::Handle) -> bool {
        handle.element().mathml_annotation_xml_integration_point
    }
}
