use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct RenderedResult<T, E> {
    error: bool,
    #[serde(flatten)]
    result: Result<T, E>,
}

impl<T, E> From<Result<T, E>> for RenderedResult<T, E> {
    fn from(result: Result<T, E>) -> Self {
        Self {
            error: result.is_err(),
            result,
        }
    }
}
