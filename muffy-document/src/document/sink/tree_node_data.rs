use super::tree_element::TreeElement;
use markup5ever::tendril::StrTendril;

pub enum TreeNodeData {
    Comment,
    Doctype,
    Document,
    Element(TreeElement),
    ProcessingInstruction,
    Text(StrTendril),
}
