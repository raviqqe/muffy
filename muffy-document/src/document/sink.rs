use super::{
    Document, Element, Node,
    namespace::{qualify_attribute_name, qualify_element_name},
};
use alloc::{borrow::Cow, sync::Arc};
use core::{
    cell::{Ref, RefCell},
    mem,
};
use markup5ever::{
    Attribute, QualName,
    interface::{ElementFlags, NodeOrText, QuirksMode, TreeSink},
    ns,
    tendril::StrTendril,
};

const DOCUMENT_HANDLE: usize = 0;

/// A tree sink building a document.
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

        if tree.nodes[*element].parent.is_some() {
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

    fn reparent_children(&self, node: &usize, new_parent: &usize) {
        let mut tree = self.tree.borrow_mut();
        let children = mem::take(&mut tree.nodes[*node].children);

        for &child in &children {
            tree.nodes[child].parent = Some(*new_parent);
        }

        tree.nodes[*new_parent].children.extend(children);
    }

    fn is_mathml_annotation_xml_integration_point(&self, handle: &usize) -> bool {
        self.tree
            .borrow()
            .element(*handle)
            .mathml_annotation_xml_integration_point
    }
}

struct Tree {
    nodes: Vec<TreeNode>,
}

impl Tree {
    fn create(&mut self, data: TreeNodeData) -> usize {
        self.nodes.push(TreeNode::new(data));
        self.nodes.len() - 1
    }

    fn element(&self, handle: usize) -> &TreeElement {
        match &self.nodes[handle].data {
            TreeNodeData::Element(element) => element,
            _ => panic!("element expected"),
        }
    }

    fn element_mut(&mut self, handle: usize) -> &mut TreeElement {
        match &mut self.nodes[handle].data {
            TreeNodeData::Element(element) => element,
            _ => panic!("element expected"),
        }
    }

    fn append(&mut self, parent: usize, child: NodeOrText<usize>) {
        self.insert(parent, self.nodes[parent].children.len(), child);
    }

    fn insert_before(&mut self, sibling: usize, child: NodeOrText<usize>) {
        if let NodeOrText::AppendNode(node) = &child {
            self.detach(*node);
        }

        let parent = self.nodes[sibling].parent.expect("parent node");

        self.insert(
            parent,
            self.nodes[parent]
                .children
                .iter()
                .position(|&node| node == sibling)
                .expect("sibling node"),
            child,
        );
    }

    fn insert(&mut self, parent: usize, index: usize, child: NodeOrText<usize>) {
        let child = match child {
            NodeOrText::AppendNode(node) => node,
            NodeOrText::AppendText(text) => {
                if let Some(previous) = index
                    .checked_sub(1)
                    .map(|index| self.nodes[parent].children[index])
                    && let TreeNodeData::Text(previous) = &mut self.nodes[previous].data
                {
                    previous.push_tendril(&text);
                    return;
                }

                self.create(TreeNodeData::Text(text))
            }
        };

        self.nodes[child].parent = Some(parent);
        self.nodes[parent].children.insert(index, child);
    }

    fn detach(&mut self, handle: usize) {
        if let Some(parent) = self.nodes[handle].parent.take() {
            self.nodes[parent].children.retain(|&child| child != handle);
        }
    }

    fn build_document(&self) -> Document {
        Document::new(self.build_nodes(&self.nodes[DOCUMENT_HANDLE].children))
    }

    fn build_nodes(&self, handles: &[usize]) -> Vec<Arc<Node>> {
        handles
            .iter()
            .flat_map(|&handle| self.build_node(handle))
            .map(Arc::new)
            .collect()
    }

    fn build_node(&self, handle: usize) -> Option<Node> {
        let node = &self.nodes[handle];

        match &node.data {
            TreeNodeData::Element(element) => Some(Node::Element(
                Element::new(
                    qualify_element_name(&element.name),
                    element
                        .attributes
                        .iter()
                        // Namespace declarations on foreign elements are not
                        // semantic attributes.
                        .filter(|attribute| attribute.name.ns != ns!(xmlns))
                        .map(|attribute| {
                            (
                                qualify_attribute_name(&attribute.name),
                                attribute.value.to_string(),
                            )
                        })
                        .collect(),
                    self.build_nodes(&node.children),
                )
                .set_namespace((!element.name.ns.is_empty()).then(|| element.name.ns.to_string())),
            )),
            TreeNodeData::Text(text) => Some(Node::Text(text.to_string())),
            TreeNodeData::Comment
            | TreeNodeData::Doctype
            | TreeNodeData::Document
            | TreeNodeData::ProcessingInstruction => None,
        }
    }
}

impl Default for Tree {
    fn default() -> Self {
        Self {
            nodes: vec![TreeNode::new(TreeNodeData::Document)],
        }
    }
}

struct TreeNode {
    data: TreeNodeData,
    parent: Option<usize>,
    children: Vec<usize>,
}

impl TreeNode {
    const fn new(data: TreeNodeData) -> Self {
        Self {
            data,
            parent: None,
            children: vec![],
        }
    }
}

enum TreeNodeData {
    Comment,
    Doctype,
    Document,
    Element(TreeElement),
    ProcessingInstruction,
    Text(StrTendril),
}

struct TreeElement {
    name: QualName,
    attributes: Vec<Attribute>,
    template_contents: Option<usize>,
    mathml_annotation_xml_integration_point: bool,
}
