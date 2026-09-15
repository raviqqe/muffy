mod tree;
mod tree_element;
mod tree_node;
mod tree_node_data;

use self::{
    tree::{DOCUMENT_HANDLE, Tree},
    tree_element::TreeElement,
    tree_node_data::TreeNodeData,
};
use super::Document;
use alloc::borrow::Cow;
use core::cell::{Ref, RefCell};
use markup5ever::{
    Attribute, QualName,
    interface::{ElementFlags, NodeOrText, QuirksMode, TreeSink},
    tendril::StrTendril,
};

/// A document sink.
#[derive(Default)]
pub(crate) struct DocumentSink {
    tree: RefCell<Tree>,
    errors: RefCell<Vec<Cow<'static, str>>>,
}

impl TreeSink for DocumentSink {
    type Handle = usize;
    type Output = (Document, Vec<Cow<'static, str>>);
    type ElemName<'a>
        = Ref<'a, QualName>
    where
        Self: 'a;

    fn finish(self) -> Self::Output {
        (
            self.tree.into_inner().build_document(),
            self.errors.into_inner(),
        )
    }

    fn parse_error(&self, message: Cow<'static, str>) {
        self.errors.borrow_mut().push(message);
    }

    fn get_document(&self) -> usize {
        DOCUMENT_HANDLE
    }

    fn elem_name<'a>(&'a self, target: &'a usize) -> Ref<'a, QualName> {
        Ref::map(self.tree.borrow(), |tree| &tree.element(*target).name)
    }

    fn create_element(
        &self,
        name: QualName,
        attributes: Vec<Attribute>,
        flags: ElementFlags,
    ) -> usize {
        let mut tree = self.tree.borrow_mut();
        let template_contents = flags.template.then(|| tree.create(TreeNodeData::Document));

        tree.create(TreeNodeData::Element(TreeElement {
            name,
            attributes,
            template_contents,
            mathml_annotation_xml_integration_point: flags.mathml_annotation_xml_integration_point,
        }))
    }

    fn create_comment(&self, _text: StrTendril) -> usize {
        self.tree.borrow_mut().create(TreeNodeData::Comment)
    }

    fn create_pi(&self, _target: StrTendril, _data: StrTendril) -> usize {
        self.tree
            .borrow_mut()
            .create(TreeNodeData::ProcessingInstruction)
    }

    fn append(&self, parent: &usize, child: NodeOrText<usize>) {
        self.tree.borrow_mut().append(*parent, child);
    }

    fn append_based_on_parent_node(
        &self,
        element: &usize,
        previous_element: &usize,
        child: NodeOrText<usize>,
    ) {
        let mut tree = self.tree.borrow_mut();

        if tree.parent(*element).is_some() {
            tree.insert_before(*element, child);
        } else {
            tree.append(*previous_element, child);
        }
    }

    fn append_doctype_to_document(
        &self,
        _name: StrTendril,
        _public_id: StrTendril,
        _system_id: StrTendril,
    ) {
        let mut tree = self.tree.borrow_mut();
        let doctype = tree.create(TreeNodeData::Doctype);

        tree.append(DOCUMENT_HANDLE, NodeOrText::AppendNode(doctype));
    }

    fn get_template_contents(&self, target: &usize) -> usize {
        self.tree
            .borrow()
            .element(*target)
            .template_contents
            .expect("template contents")
    }

    fn same_node(&self, one: &usize, other: &usize) -> bool {
        one == other
    }

    fn set_quirks_mode(&self, _mode: QuirksMode) {}

    fn append_before_sibling(&self, sibling: &usize, child: NodeOrText<usize>) {
        self.tree.borrow_mut().insert_before(*sibling, child);
    }

    fn add_attrs_if_missing(&self, target: &usize, mut attributes: Vec<Attribute>) {
        let mut tree = self.tree.borrow_mut();
        let element = tree.element_mut(*target);

        attributes.retain(|attribute| {
            element
                .attributes
                .iter()
                .all(|other| other.name != attribute.name)
        });
        element.attributes.extend(attributes);
    }

    fn remove_from_parent(&self, target: &usize) {
        self.tree.borrow_mut().detach(*target);
    }

    // spell-checker: disable-next-line
    fn reparent_children(&self, node: &usize, new_parent: &usize) {
        self.tree.borrow_mut().move_children(*node, *new_parent);
    }

    fn is_mathml_annotation_xml_integration_point(&self, handle: &usize) -> bool {
        self.tree
            .borrow()
            .element(*handle)
            .mathml_annotation_xml_integration_point
    }
}
