mod options;

pub use self::options::{RenderFormat, RenderOptions};
use crate::{DocumentOutput, error::Error};
use colored::Colorize;
use tokio::io::{AsyncWrite, AsyncWriteExt};

/// Renders a result of document validation.
pub async fn render_document(
    document: &DocumentOutput,
    options: &RenderOptions,
    mut writer: (impl AsyncWrite + Unpin),
) -> Result<(), Error> {
    if !options.verbose()
        && document
            .elements()
            .all(|element| element.results().all(Result::is_ok))
    {
        return Ok(());
    }

    render_line(
        &format!("{}", document.url().to_string().yellow()),
        &mut writer,
    )
    .await?;

    for output in document.elements() {
        if !options.verbose() && output.results().all(Result::is_ok) {
            continue;
        }

        render_line(
            &format!(
                "\t{} {}",
                output.element().name(),
                output
                    .element()
                    .attributes()
                    .iter()
                    .map(|(key, value)| format!("{key}=\"{value}\""))
                    .collect::<Vec<_>>()
                    .join(" "),
            ),
            &mut writer,
        )
        .await?;

        for result in output.results() {
            match result {
                Ok(success) => {
                    if !options.verbose() {
                        continue;
                    }

                    render_line(
                        &success.response().map_or_else(
                            || "\t\tvalid URL".into(),
                            |response| {
                                format!(
                                    "\t\t{}\t{}\t{}",
                                    response.status().to_string().green(),
                                    response.url(),
                                    format!("{} ms", response.duration().as_millis()).yellow()
                                )
                            },
                        ),
                        &mut writer,
                    )
                    .await?
                }
                Err(error) => {
                    render_line(&format!("\t\t{}\t{error}", "ERROR".red()), &mut writer).await?
                }
            }
        }
    }

    Ok(())
}

async fn render_line(string: &str, writer: &mut (impl AsyncWrite + Unpin)) -> Result<(), Error> {
    writer.write_all(string.as_bytes()).await?;
    writer.write_all(b"\n").await?;

    Ok(())
}
