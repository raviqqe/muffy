use crate::value::Value;

pub struct Attribute {
    pub name: &'static str,
    pub value: &'static Value,
}
