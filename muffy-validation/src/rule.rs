use crate::variant::Variant;

pub struct Rule {
    pub attributes: &'static [&'static str],
    pub children: &'static [&'static str],
    pub variants: &'static [Variant],
}
