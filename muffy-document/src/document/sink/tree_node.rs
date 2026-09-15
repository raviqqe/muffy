use super::{tree_element::TreeElement, tree_node_data::TreeNodeData};
use crate::document::{
    Element, Node,
    namespace::{qualify_attribute_name, qualify_element_name},
};
use alloc::sync::Arc;
use core::{cell::Cell, iter::successors};
use markup5ever::ns;

pub struct TreeNode<'a> {
    pub data: TreeNodeData<'a>,
    parent: Cell<Option<&'a Self>>,
    previous_sibling: Cell<Option<&'a Self>>,
    next_sibling: Cell<Option<&'a Self>>,
    first_child: Cell<Option<&'a Self>>,
    last_child: Cell<Option<&'a Self>>,
}

impl<'a> TreeNode<'a> {
    pub const fn new(data: TreeNodeData<'a>) -> Self {
        Self {
            data,
            parent: Cell::new(None),
            previous_sibling: Cell::new(None),
            next_sibling: Cell::new(None),
            first_child: Cell::new(None),
            last_child: Cell::new(None),
        }
    }

    pub const fn parent(&self) -> Option<&'a Self> {
        self.parent.get()
    }

    pub const fn previous_sibling(&self) -> Option<&'a Self> {
        self.previous_sibling.get()
    }

    pub const fn last_child(&self) -> Option<&'a Self> {
        self.last_child.get()
    }

    pub fn element(&self) -> &TreeElement<'a> {
        match &self.data {
            TreeNodeData::Element(element) => element,
            _ => panic!("element expected"),
        }
    }

    pub fn append(&'a self, child: &'a Self) {
        child.detach();
        child.parent.set(Some(self));

        if let Some(last_child) = self.last_child.get() {
            last_child.next_sibling.set(Some(child));
            child.previous_sibling.set(Some(last_child));
        } else {
            self.first_child.set(Some(child));
        }

        self.last_child.set(Some(child));
    }

    pub fn insert_before(&'a self, sibling: &'a Self) {
        self.detach();
        self.parent.set(sibling.parent.get());
        self.next_sibling.set(Some(sibling));

        if let Some(previous_sibling) = sibling.previous_sibling.get() {
            previous_sibling.next_sibling.set(Some(self));
            self.previous_sibling.set(Some(previous_sibling));
        } else if let Some(parent) = sibling.parent.get() {
            parent.first_child.set(Some(self));
        }

        sibling.previous_sibling.set(Some(self));
    }

    pub fn detach(&self) {
        let parent = self.parent.take();
        let previous_sibling = self.previous_sibling.take();
        let next_sibling = self.next_sibling.take();

        if let Some(previous_sibling) = previous_sibling {
            previous_sibling.next_sibling.set(next_sibling);
        } else if let Some(parent) = parent {
            parent.first_child.set(next_sibling);
        }

        if let Some(next_sibling) = next_sibling {
            next_sibling.previous_sibling.set(previous_sibling);
        } else if let Some(parent) = parent {
            parent.last_child.set(previous_sibling);
        }
    }

    pub fn move_children(&self, new_parent: &'a Self) {
        while let Some(child) = self.first_child.get() {
            new_parent.append(child);
        }
    }

    pub fn build_children(&self) -> Vec<Arc<Node>> {
        successors(self.first_child.get(), |node| node.next_sibling.get())
            .flat_map(Self::build)
            .map(Arc::new)
            .collect()
    }

    fn build(&self) -> Option<Node> {
        match &self.data {
            TreeNodeData::Element(element) => Some(Node::Element(
                Element::new(
                    qualify_element_name(&element.name),
                    element
                        .attributes
                        .borrow()
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
                    self.build_children(),
                )
                .set_namespace((!element.name.ns.is_empty()).then(|| element.name.ns.to_string())),
            )),
            TreeNodeData::Text(text) => Some(Node::Text(text.borrow().to_string())),
            TreeNodeData::Comment
            | TreeNodeData::Doctype
            | TreeNodeData::Document
            | TreeNodeData::ProcessingInstruction => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::cell::RefCell;
    use pretty_assertions::assert_eq;
    use typed_arena::Arena;

    fn create_parent<'a>(arena: &'a Arena<TreeNode<'a>>) -> &'a TreeNode<'a> {
        arena.alloc(TreeNode::new(TreeNodeData::Document))
    }

    fn create_text<'a>(arena: &'a Arena<TreeNode<'a>>, value: &str) -> &'a TreeNode<'a> {
        arena.alloc(TreeNode::new(TreeNodeData::Text(RefCell::new(
            value.into(),
        ))))
    }

    fn text(value: &str) -> Arc<Node> {
        Arc::new(Node::Text(value.into()))
    }

    #[test]
    fn append_children() {
        let arena = Arena::new();
        let parent = create_parent(&arena);

        parent.append(create_text(&arena, "foo"));
        parent.append(create_text(&arena, "bar"));

        assert_eq!(parent.build_children(), vec![text("foo"), text("bar")]);
    }

    #[test]
    fn append_attached_node() {
        let arena = Arena::new();
        let parent = create_parent(&arena);
        let other = create_parent(&arena);
        let node = create_text(&arena, "foo");

        parent.append(node);
        parent.append(create_text(&arena, "bar"));
        other.append(node);

        assert_eq!(parent.build_children(), vec![text("bar")]);
        assert_eq!(other.build_children(), vec![text("foo")]);
    }

    #[test]
    fn insert_before_first_child() {
        let arena = Arena::new();
        let parent = create_parent(&arena);
        let sibling = create_text(&arena, "bar");

        parent.append(sibling);
        create_text(&arena, "foo").insert_before(sibling);
        parent.append(create_text(&arena, "baz"));

        assert_eq!(
            parent.build_children(),
            vec![text("foo"), text("bar"), text("baz")]
        );
    }

    #[test]
    fn insert_before_last_child() {
        let arena = Arena::new();
        let parent = create_parent(&arena);
        let sibling = create_text(&arena, "baz");

        parent.append(create_text(&arena, "foo"));
        parent.append(sibling);
        create_text(&arena, "bar").insert_before(sibling);

        assert_eq!(
            parent.build_children(),
            vec![text("foo"), text("bar"), text("baz")]
        );
    }

    #[test]
    fn insert_last_child_before_first_child() {
        let arena = Arena::new();
        let parent = create_parent(&arena);
        let first = create_text(&arena, "foo");
        let last = create_text(&arena, "baz");

        parent.append(first);
        parent.append(create_text(&arena, "bar"));
        parent.append(last);
        last.insert_before(first);
        parent.append(create_text(&arena, "qux"));

        assert_eq!(
            parent.build_children(),
            vec![text("baz"), text("foo"), text("bar"), text("qux")]
        );
    }

    #[test]
    fn detach_first_child() {
        let arena = Arena::new();
        let parent = create_parent(&arena);
        let first = create_text(&arena, "foo");
        let second = create_text(&arena, "bar");

        parent.append(first);
        parent.append(second);
        first.detach();
        create_text(&arena, "baz").insert_before(second);

        assert_eq!(parent.build_children(), vec![text("baz"), text("bar")]);
        assert!(first.parent().is_none());
    }

    #[test]
    fn detach_middle_child() {
        let arena = Arena::new();
        let parent = create_parent(&arena);
        let middle = create_text(&arena, "bar");

        parent.append(create_text(&arena, "foo"));
        parent.append(middle);
        parent.append(create_text(&arena, "baz"));
        middle.detach();

        assert_eq!(parent.build_children(), vec![text("foo"), text("baz")]);
    }

    #[test]
    fn detach_last_child() {
        let arena = Arena::new();
        let parent = create_parent(&arena);
        let last = create_text(&arena, "bar");

        parent.append(create_text(&arena, "foo"));
        parent.append(last);
        last.detach();
        parent.append(create_text(&arena, "baz"));

        assert_eq!(parent.build_children(), vec![text("foo"), text("baz")]);
    }

    #[test]
    fn detach_only_child() {
        let arena = Arena::new();
        let parent = create_parent(&arena);
        let child = create_text(&arena, "foo");

        parent.append(child);
        child.detach();

        assert_eq!(parent.build_children(), vec![]);

        parent.append(create_text(&arena, "bar"));

        assert_eq!(parent.build_children(), vec![text("bar")]);
    }

    #[test]
    fn move_children() {
        let arena = Arena::new();
        let parent = create_parent(&arena);
        let new_parent = create_parent(&arena);

        parent.append(create_text(&arena, "foo"));
        parent.append(create_text(&arena, "bar"));
        new_parent.append(create_text(&arena, "baz"));
        parent.move_children(new_parent);
        parent.append(create_text(&arena, "qux"));

        assert_eq!(parent.build_children(), vec![text("qux")]);
        assert_eq!(
            new_parent.build_children(),
            vec![text("baz"), text("foo"), text("bar")]
        );
    }
}
