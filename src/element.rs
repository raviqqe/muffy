pub struct Element {
    name: String,
    attributes: Vec<(String, String)>,
}

impl Element {
    pub const fn new(name: String, attributes: Vec<(String, String)>) -> Self {
        Self { name, attributes }
    }

    #[allow(clippy::missing_const_for_fn)]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[allow(clippy::missing_const_for_fn)]
    pub fn attributes(&self) -> &[(String, String)] {
        &self.attributes
    }
}
