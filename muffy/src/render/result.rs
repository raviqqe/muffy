use super::response::RenderedResponse;
use crate::item_output::ItemOutput;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct RenderedResult<T, E> {
    error: bool,
    #[serde(flatten)]
    output: Result<T, E>,
}

impl<T, E> RenderedResult<T, E> {
    pub const fn response(&self) -> Option<&RenderedResponse<'a>> {
        self.response.as_ref()
    }
}

impl<T, E> From<Result<T, E>> for RenderedResult<T, E> {}
