use crate::{Document, error::Error};
use colored::Colorize;
use tokio::io::{AsyncWrite, AsyncWriteExt};

/// A rendering format.
#[derive(Clone, Copy, Debug)]
pub enum RenderFormat {
    // JSON.
    Json,
    // Human-readable text.
    Text,
}

/// Rendering options.
#[derive(Clone, Copy, Debug)]
pub struct RenderOptions {
    format: RenderFormat,
    verbose: bool,
}

impl RenderOptions {
    /// Creates a new `RenderOptions` instance.
    pub fn new(format: RenderFormat, verbose: bool) -> Self {
        Self { format, verbose }
    }

    /// Returns the rendering format.
    pub fn format(&self) -> &RenderFormat {
        &self.format
    }

    /// Returns whether verbose output is enabled.
    pub fn verbose(&self) -> bool {
        self.verbose
    }
}

/// Renders a result of document validation.
pub async fn render_document(
    document: &Document,
    verbose: bool,
    mut writer: (impl AsyncWrite + Unpin),
) -> Result<(), Error> {
    if !verbose
        && document
            .elements()
            .all(|(_, results)| results.iter().all(Result::is_ok))
    {
        return Ok(());
    }

    render_line(
        &format!("{}", document.url().to_string().yellow()),
        &mut writer,
    )
    .await?;

    for (element, results) in document.elements() {
        if !verbose && results.iter().all(Result::is_ok) {
            continue;
        }

        render_line(
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
            &mut writer,
        )
        .await?;

        for result in results {
            match result {
                Ok(success) => {
                    if !verbose {
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
