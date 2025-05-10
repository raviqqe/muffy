use super::response::RenderedResponse;
use crate::success::ItemOutput;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct RenderedItemOutput<'a> {
    response: Option<RenderedResponse<'a>>,
}

impl<'a> RenderedItemOutput<'a> {
    pub const fn response(&self) -> Option<&RenderedResponse<'a>> {
        self.response.as_ref()
    }
}

impl<'a> From<&'a ItemOutput> for RenderedItemOutput<'a> {
    fn from(success: &'a ItemOutput) -> Self {
        Self {
            response: success.response().map(RenderedResponse::from),
        }
    }
}
