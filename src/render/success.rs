use super::response::RenderedResponse;
use crate::success::Success;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct RenderedSuccess<'a> {
    response: Option<RenderedResponse<'a>>,
}

impl<'a> RenderedSuccess<'a> {
    pub fn response(&self) -> Option<&RenderedResponse<'a>> {
        self.response.as_ref()
    }
}

impl<'a> From<&'a Success> for RenderedSuccess<'a> {
    fn from(success: &'a Success) -> Self {
        Self {
            response: success
                .response()
                .map(|response| RenderedResponse::from(response)),
        }
    }
}
