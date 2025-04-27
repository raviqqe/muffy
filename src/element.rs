pub struct Element {
    name: String,
    attributes: Vec<(String, String)>,
}

impl Element {
    pub fn new(name: String, attributes: Vec<(String, String)>) -> Self {
        Self { name, attributes }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn attributes(&self) -> &[(String, String)] {
        &self.attributes
    }
}
