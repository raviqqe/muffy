use crate::{context::Context, error::Error};

pub async fn validate_link(_context: &Context, link: &str) -> Result<(), Error> {
    let response = reqwest::get(link)
        .await
        .map_err(|error| Error::Get(link, error));

    Ok(())
}
