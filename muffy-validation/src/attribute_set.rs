use crate::attribute::Attribute;

pub struct AttributeSet {
    pub required: &'static [Attribute],
    pub optional: &'static [Attribute],
}

impl AttributeSet {
    pub fn find(&self, name: &str) -> Option<&'static Attribute> {
        [self.required, self.optional]
            .into_iter()
            .find_map(|attributes| {
                attributes
                    .binary_search_by(|attribute| attribute.name.cmp(name))
                    .ok()
                    .map(|index| &attributes[index])
            })
    }
}
