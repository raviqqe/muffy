use crate::{attribute_set::AttributeSet, content::Content};

pub struct Variant {
    pub attributes: &'static [AttributeSet],
    pub content: &'static Content,
}
