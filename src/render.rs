use crate::error::Error;
use crate::{context::Context, response::Response};
use colored::Colorize;
use tokio::io::{AsyncWriteExt, Stdout};
use url::Url;

pub async fn render(
    context: &Context,
    url: &Url,
    results: &[Result<Response, Error>],
) -> Result<(), Error> {
    let mut stdout = context.stdout().lock().await;

    render_line(&mut stdout, &format!("{}", url.to_string().yellow())).await?;

    for result in results {
        match result {
            Ok(response) => {
                render_line(
                    &mut stdout,
                    &format!(
                        "  {} {} {} ({} ms)",
                        "OK".green(),
                        response.status(),
                        response.url(),
                        response.duration().as_millis()
                    ),
                )
                .await?
            }
            Err(error) => render_line(&mut stdout, &format!("  {} {error}", "ERROR".red())).await?,
        }
    }

    Ok(())
}

async fn render_line(stdout: &mut Stdout, string: &str) -> Result<(), Error> {
    stdout.write_all(string.as_bytes()).await?;
    stdout.write_all(b"\n").await?;

    Ok(())
}
