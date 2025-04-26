use crate::{context::Context, error::Error};

pub async fn validate_link(_context: &Context, url: &str) -> Result<(), Error> {
    let response = reqwest::get(url).await.map_err(|source| Error::Get {
        url: url.into(),
        source,
    })?;

    Ok(())
}
