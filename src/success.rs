use crate::response::{Response, SerializedResponse};
use alloc::sync::Arc;
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
#[serde(into = "SerializedSuccess")]
pub struct Success {
    response: Option<Arc<Response>>,
}

impl Default for Success {
    fn default() -> Self {
        Self::new()
    }
}

impl Success {
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

// TODO Move this under the `render` module.
#[derive(Serialize)]
struct SerializedSuccess {
    response: Option<SerializedResponse>,
}

impl From<Success> for SerializedSuccess {
    fn from(success: Success) -> Self {
        Self {
            response: success
                .response
                .map(|response| SerializedResponse::from_response(&response)),
        }
    }
}
