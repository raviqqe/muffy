use super::{tree_element::TreeElement, tree_node::TreeNode, tree_node_data::TreeNodeData};
use crate::document::{
    Document, Element, Node,
    namespace::{qualify_attribute_name, qualify_element_name},
};
use alloc::sync::Arc;
use core::mem;
use markup5ever::{interface::NodeOrText, ns};

pub const DOCUMENT_HANDLE: usize = 0;

pub struct Tree {
    nodes: Vec<TreeNode>,
}

impl Tree {
    pub fn create(&mut self, data: TreeNodeData) -> usize {
        self.nodes.push(TreeNode::new(data));
        self.nodes.len() - 1
    }

    pub fn parent(&self, handle: usize) -> Option<usize> {
        self.nodes[handle].parent
    }

    pub fn element(&self, handle: usize) -> &TreeElement {
        match &self.nodes[handle].data {
            TreeNodeData::Element(element) => element,
            _ => panic!("element expected"),
        }
    }

    pub fn element_mut(&mut self, handle: usize) -> &mut TreeElement {
        match &mut self.nodes[handle].data {
            TreeNodeData::Element(element) => element,
            _ => panic!("element expected"),
        }
    }

    pub fn append(&mut self, parent: usize, child: NodeOrText<usize>) {
        self.insert(parent, self.nodes[parent].children.len(), child);
    }

    pub fn insert_before(&mut self, sibling: usize, child: NodeOrText<usize>) {
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

    pub fn detach(&mut self, handle: usize) {
        if let Some(parent) = self.nodes[handle].parent.take() {
            self.nodes[parent].children.retain(|&child| child != handle);
        }
    }

    pub fn move_children(&mut self, node: usize, new_parent: usize) {
        let children = mem::take(&mut self.nodes[node].children);

        for &child in &children {
            self.nodes[child].parent = Some(new_parent);
        }

        self.nodes[new_parent].children.extend(children);
    }

    pub fn build_document(&self) -> Document {
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
