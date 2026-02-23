use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub(crate) enum RenderedResult<T, E> {
    Ok(T),
    Err(RenderedError<E>),
}

impl<T, E> RenderedResult<T, E> {
    pub const fn is_ok(&self) -> bool {
        matches!(self, Self::Ok(_))
    }

    pub const fn is_err(&self) -> bool {
        matches!(self, Self::Err(_))
    }

    pub const fn result(&self) -> Result<&T, &E> {
        match &self {
            Self::Ok(value) => Ok(value),
            Self::Err(RenderedError { error }) => Err(error),
        }
    }
}

impl<T, E> From<Result<T, E>> for RenderedResult<T, E> {
    fn from(result: Result<T, E>) -> Self {
        match result {
            Ok(value) => Self::Ok(value),
            Err(error) => Self::Err(RenderedError { error }),
        }
    }
}

#[derive(Debug, Serialize)]
pub(crate) struct RenderedError<E> {
    error: E,
}
