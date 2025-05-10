use crate::response::Response;
use alloc::sync::Arc;
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct ItemOutput {
    response: Option<Arc<Response>>,
}

impl Default for ItemOutput {
    fn default() -> Self {
        Self::new()
    }
}

impl ItemOutput {
    pub const fn new() -> Self {
        Self { response: None }
    }

    pub fn response(&self) -> Option<&Response> {
        self.response.as_deref()
    }

    pub fn with_response(mut self, response: Arc<Response>) -> Self {
        self.response = Some(response);
        self
    }
}
