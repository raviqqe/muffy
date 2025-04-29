use crate::{context::Context, element::Element, error::Error, response::Response};
use alloc::sync::Arc;
use colored::Colorize;
use tokio::io::{AsyncWriteExt, Stdout};
use url::Url;

// TODO Render results as JSON.
pub async fn render(
    context: &Context,
    url: &Url,
    results: impl IntoIterator<Item = (&Element, &Result<Arc<Response>, Error>)>,
) -> Result<(), Error> {
    let mut stdout = context.stdout().lock().await;

    render_line(&mut stdout, &format!("{}", url.to_string().yellow())).await?;

    for (element, result) in results {
        render_line(
            &mut stdout,
            &format!(
                "\t{} {}",
                element.name(),
                element
                    .attributes()
                    .iter()
                    .map(|(key, value)| format!("{key}=\"{value}\""))
                    .collect::<Vec<_>>()
                    .join(" "),
            ),
        )
        .await?;

        match result {
            Ok(response) => {
                render_line(
                    &mut stdout,
                    &format!(
                        "\t\t{}\t{}\t{}",
                        response.status().to_string().green(),
                        response.url(),
                        format!("{} ms", response.duration().as_millis()).yellow()
                    ),
                )
                .await?
            }
            Err(error) => {
                render_line(&mut stdout, &format!("\t\t{}\t{error}", "ERROR".red())).await?
            }
        }
    }

    Ok(())
}

async fn render_line(stdout: &mut Stdout, string: &str) -> Result<(), Error> {
    stdout.write_all(string.as_bytes()).await?;
    stdout.write_all(b"\n").await?;

    Ok(())
}
