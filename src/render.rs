mod document_output;
mod options;

pub use self::options::{RenderFormat, RenderOptions};
use crate::{DocumentOutput, error::Error};
use colored::Colorize;
use core::pin::pin;
use tokio::io::{AsyncWrite, AsyncWriteExt};

/// Renders a result of document validation.
pub async fn render_document(
    mut document: DocumentOutput,
    options: &RenderOptions,
    mut writer: impl AsyncWrite,
) -> Result<(), Error> {
    let mut writer = pin!(writer);

    if !options.verbose() {
        document.retain_error();
    }

    if !options.verbose()
        && document
            .elements()
            .all(|element| element.results().all(Result::is_ok))
    {
        return Ok(());
    }

    if options.format() == RenderFormat::Json {
        return render_json_document(document, &mut writer).await;
    }

    render_line(
        &format!("{}", document.url().to_string().yellow()),
        &mut writer,
    )
    .await?;

    for output in document.elements() {
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

pub async fn render_json_document(
    document: DocumentOutput,
    writer: &mut (impl AsyncWrite + Unpin),
) -> Result<(), Error> {
    render_line(&serde_json::to_string(&document)?, writer).await
}

async fn render_line(string: &str, writer: &mut (impl AsyncWrite + Unpin)) -> Result<(), Error> {
    writer.write_all(string.as_bytes()).await?;
    writer.write_all(b"\n").await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        element::Element, element_output::ElementOutput, response::Response, success::Success,
    };
    use core::str;
    use insta::assert_snapshot;
    use std::io::{self, ErrorKind};
    use url::Url;

    fn mixed_document_output() -> DocumentOutput {
        DocumentOutput::new(
            Url::parse("https://foo.com").unwrap(),
            vec![ElementOutput::new(
                Element::new("a".into(), vec![]),
                vec![
                    Ok(Success::default().with_response(
                        Response::new(
                            Url::parse("https://foo.com").unwrap(),
                            Default::default(),
                            Default::default(),
                            Default::default(),
                            Default::default(),
                        )
                        .into(),
                    )),
                    Err(Error::HtmlParse(io::Error::new(ErrorKind::NotFound, "foo"))),
                ],
            )],
        )
    }

    fn successful_document_output() -> DocumentOutput {
        DocumentOutput::new(
            Url::parse("https://foo.com").unwrap(),
            vec![ElementOutput::new(
                Element::new("a".into(), vec![]),
                vec![Ok(Success::default().with_response(
                    Response::new(
                        Url::parse("https://foo.com").unwrap(),
                        Default::default(),
                        Default::default(),
                        Default::default(),
                        Default::default(),
                    )
                    .into(),
                ))],
            )],
        )
    }

    mod text {
        use super::*;

        #[tokio::test]
        async fn render() {
            colored::control::set_override(false);
            let mut string = vec![];

            render_document(
                mixed_document_output(),
                &RenderOptions::default(),
                &mut string,
            )
            .await
            .unwrap();

            assert_snapshot!(str::from_utf8(&string).unwrap());
        }

        #[tokio::test]
        async fn render_with_verbose_option() {
            colored::control::set_override(false);
            let mut string = vec![];

            render_document(
                mixed_document_output(),
                &RenderOptions::default().set_verbose(true),
                &mut string,
            )
            .await
            .unwrap();

            assert_snapshot!(str::from_utf8(&string).unwrap());
        }

        #[tokio::test]
        async fn render_successful_document() {
            colored::control::set_override(false);
            let mut string = vec![];

            render_document(
                successful_document_output(),
                &RenderOptions::default(),
                &mut string,
            )
            .await
            .unwrap();

            assert_snapshot!(str::from_utf8(&string).unwrap());
        }

        #[tokio::test]
        async fn render_successful_element() {
            colored::control::set_override(false);
            let mut string = vec![];

            render_document(
                DocumentOutput::new(
                    Url::parse("https://foo.com").unwrap(),
                    vec![
                        ElementOutput::new(
                            Element::new("a".into(), vec![]),
                            vec![Ok(Success::default().with_response(
                                Response::new(
                                    Url::parse("https://foo.com").unwrap(),
                                    Default::default(),
                                    Default::default(),
                                    Default::default(),
                                    Default::default(),
                                )
                                .into(),
                            ))],
                        ),
                        ElementOutput::new(
                            Element::new("a".into(), vec![]),
                            vec![Err(Error::HtmlParse(io::Error::new(
                                ErrorKind::NotFound,
                                "foo",
                            )))],
                        ),
                    ],
                ),
                &RenderOptions::default(),
                &mut string,
            )
            .await
            .unwrap();

            assert_snapshot!(str::from_utf8(&string).unwrap());
        }
    }

    mod json {
        use super::*;

        #[tokio::test]
        async fn render() {
            let mut string = vec![];

            render_document(
                mixed_document_output(),
                &RenderOptions::default().set_format(RenderFormat::Json),
                &mut string,
            )
            .await
            .unwrap();

            assert_snapshot!(str::from_utf8(&string).unwrap());
        }

        #[tokio::test]
        async fn render_with_verbose_option() {
            let mut string = vec![];

            render_document(
                mixed_document_output(),
                &RenderOptions::default()
                    .set_format(RenderFormat::Json)
                    .set_verbose(true),
                &mut string,
            )
            .await
            .unwrap();

            assert_snapshot!(str::from_utf8(&string).unwrap());
        }

        #[tokio::test]
        async fn render_successful_document() {
            colored::control::set_override(false);
            let mut string = vec![];

            render_document(
                successful_document_output(),
                &RenderOptions::default().set_format(RenderFormat::Json),
                &mut string,
            )
            .await
            .unwrap();

            assert_snapshot!(str::from_utf8(&string).unwrap());
        }

        #[tokio::test]
        async fn render_successful_element() {
            colored::control::set_override(false);
            let mut string = vec![];

            render_document(
                DocumentOutput::new(
                    Url::parse("https://foo.com").unwrap(),
                    vec![
                        ElementOutput::new(
                            Element::new("a".into(), vec![]),
                            vec![Ok(Success::default().with_response(
                                Response::new(
                                    Url::parse("https://foo.com").unwrap(),
                                    Default::default(),
                                    Default::default(),
                                    Default::default(),
                                    Default::default(),
                                )
                                .into(),
                            ))],
                        ),
                        ElementOutput::new(
                            Element::new("a".into(), vec![]),
                            vec![Err(Error::HtmlParse(io::Error::new(
                                ErrorKind::NotFound,
                                "foo",
                            )))],
                        ),
                    ],
                ),
                &RenderOptions::default().set_format(RenderFormat::Json),
                &mut string,
            )
            .await
            .unwrap();

            assert_snapshot!(str::from_utf8(&string).unwrap());
        }
    }
}
