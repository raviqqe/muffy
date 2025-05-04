use super::response::RenderedResponse;
use serde::Serialize;

#[derive(Serialize)]
struct RenderedSuccess {
    response: Option<RenderedResponse>,
}

impl From<Success> for RenderedSuccess {
    fn from(success: Success) -> Self {
        Self {
            response: success
                .response
                .map(|response| RenderedResponse::from_response(&response)),
        }
    }
}
