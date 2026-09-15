use super::tree_element::TreeElement;
use core::cell::RefCell;
use markup5ever::tendril::StrTendril;

pub enum TreeNodeData<'a> {
    Comment,
    Doctype,
    Document,
    Element(TreeElement<'a>),
    ProcessingInstruction,
    Text(RefCell<StrTendril>),
}
