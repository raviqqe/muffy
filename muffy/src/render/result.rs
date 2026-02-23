use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct RenderedResult<T, E> {
    error: bool,
    #[serde(flatten)]
    result: Result<T, E>,
}

impl<T, E> RenderedResult<T, E> {
    pub const fn is_ok(&self) -> bool {
        !self.error
    }

    pub const fn is_err(&self) -> bool {
        self.error
    }

    pub const fn result(&self) -> &Result<T, E> {
        &self.result
    }
}

impl<T, E> From<Result<T, E>> for RenderedResult<T, E> {
    fn from(result: Result<T, E>) -> Self {
        Self {
            error: result.is_err(),
            result,
        }
    }
}
