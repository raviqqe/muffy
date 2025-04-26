use crate::context::Context;
use crate::error::Error;
use url::Url;

pub async fn render(
    context: &Context,
    url: &Url,
    results: &[Result<(), Error>],
) -> Result<(), Error> {
    Ok(())
}
