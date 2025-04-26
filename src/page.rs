use crate::{context::Context, error::Error};

pub async fn validate_link(_context: &Context, _link: &str) -> Result<(), Error> {
    Ok(())
}
