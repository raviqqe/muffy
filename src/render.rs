use crate::context::Context;
use crate::error::Error;
use tokio::io::AsyncWriteExt;
use url::Url;

pub async fn render(
    context: &Context,
    url: &Url,
    results: &[Result<(), Error>],
) -> Result<(), Error> {
    for result in results {
        match result {
            Ok(()) => render_line("OK").await?,
            Err(error) => render_line(format!("ERROR {error}")).await?,
        }
    }

    Ok(())
}

async fn render_line(context: &Context, string: &str) -> Result<(), Error> {
    context
        .stdout()
        .lock()
        .await
        .write_all(format!("{}\n", string).as_bytes())
        .await?;

    Ok(())
}
