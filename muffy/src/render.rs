mod document_output;
mod element;
mod element_output;
mod item_output;
mod options;
mod response;
mod result;
mod utility;

pub use self::options::{RenderFormat, RenderOptions};
use self::{document_output::RenderedDocumentOutput, result::RenderedResult};
use crate::{DocumentOutput, error::Error};
use colored::Colorize;
use core::pin::pin;
use tokio::io::{AsyncWrite, AsyncWriteExt};

/// Renders a result of document validation.
pub async fn render_document(
    document: &DocumentOutput,
    options: &RenderOptions,
    writer: impl AsyncWrite,
) -> Result<(), Error> {
    let mut document = RenderedDocumentOutput::from(document);
    let mut writer = pin!(writer);

    if !options.verbose() {
        document.retain_error();
    }

    if !options.verbose()
        && document
            .elements()
            .all(|element| element.results().all(RenderedResult::is_ok))
    {
        return Ok(());
    }

    if options.format() == RenderFormat::Json {
        return render_json_document(&document, &mut writer).await;
    }

    render_line(&format!("{}", document.url().yellow()), &mut writer).await?;

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
            match result.result() {
                Ok(success) => {
                    render_line(
                        &format!(
                            "\t\t✅ {}",
                            success.response().map_or_else(
                                || "valid URL".into(),
                                |response| {
                                    format!(
                                        "{}\t{}\t{}",
                                        response.status().to_string().green(),
                                        response.url(),
                                        format!("{} ms", response.duration()).yellow()
                                    )
                                },
                            )
                        ),
                        &mut writer,
                    )
                    .await?
                }
                Err(error) => {
                    render_line(&format!("\t\t❌ {}", error.to_string().red()), &mut writer).await?
                }
            }
        }
    }

    Ok(())
}

pub async fn render_json_document(
    document: &RenderedDocumentOutput<'_>,
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
        element::Element, element_output::ElementOutput, error::ItemError, item_output::ItemOutput,
        response::Response,
    };
    use core::str;
    use insta::assert_snapshot;
    use muffy_validation::{AttributeError, ChildError, MarkupError};
    use url::Url;

    fn mixed_document_output() -> DocumentOutput {
        DocumentOutput::new(
            Url::parse("https://foo.com").unwrap(),
            vec![ElementOutput::new(
                Element::new("foo".into(), vec![]),
                vec![
                    Ok(ItemOutput::default().with_response(
                        Response::new(
                            Url::parse("https://foo.com").unwrap(),
                            Default::default(),
                            Default::default(),
                            Default::default(),
                            Default::default(),
                        )
                        .into(),
                    )),
                    Err(ItemError::Markup(MarkupError::UnknownTag("foo".into()))),
                    Err(ItemError::Markup(MarkupError::InvalidElement {
                        invalid_attributes: [("bar".into(), [AttributeError::Conflict].into())]
                            .into(),
                        invalid_children: [("baz".into(), [ChildError::Misplaced].into())].into(),
                        missing_attributes: ["qux".into()].into(),
                        missing_children: ["quux".into()].into(),
                    })),
                ],
            )],
        )
    }

    fn successful_document_output() -> DocumentOutput {
        DocumentOutput::new(
            Url::parse("https://foo.com").unwrap(),
            vec![ElementOutput::new(
                Element::new("a".into(), vec![]),
                vec![Ok(ItemOutput::default().with_response(
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

    fn data_document_output() -> DocumentOutput {
        DocumentOutput::new(
            Url::parse("data:image/svg+xml,<svg xmlns='http://www.w3.org/2000/svg'><foo/></svg>")
                .unwrap(),
            vec![ElementOutput::new(
                Element::new(
                    "image".into(),
                    vec![(
                        "href".into(),
                        "data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciLz4=".into(),
                    )],
                ),
                vec![
                    Ok(ItemOutput::default().with_response(
                        Response::new(
                            Url::parse("data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciLz4=")
                                .unwrap(),
                            Default::default(),
                            Default::default(),
                            Default::default(),
                            Default::default(),
                        )
                        .into(),
                    )),
                    Err(ItemError::Markup(MarkupError::UnknownTag("foo".into()))),
                ],
            )],
        )
    }

    mod json {
        use super::*;

        #[tokio::test]
        async fn render_error() {
            let mut string = vec![];

            render_document(
                &mixed_document_output(),
                &RenderOptions::default().set_format(RenderFormat::Json),
                &mut string,
            )
            .await
            .unwrap();

            assert_snapshot!(str::from_utf8(&string).unwrap());
        }

        #[tokio::test]
        async fn render_success_and_error_with_verbose_option() {
            let mut string = vec![];

            render_document(
                &mixed_document_output(),
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
        async fn render_success() {
            let mut string = vec![];

            render_document(
                &successful_document_output(),
                &RenderOptions::default().set_format(RenderFormat::Json),
                &mut string,
            )
            .await
            .unwrap();

            assert_snapshot!(str::from_utf8(&string).unwrap());
        }

        #[tokio::test]
        async fn render_success_with_verbose_option() {
            let mut string = vec![];

            render_document(
                &successful_document_output(),
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
        async fn render_data_urls_without_bodies() {
            let mut string = vec![];

            render_document(
                &data_document_output(),
                &RenderOptions::default()
                    .set_format(RenderFormat::Json)
                    .set_verbose(true),
                &mut string,
            )
            .await
            .unwrap();

            assert_snapshot!(str::from_utf8(&string).unwrap());
        }
    }

    mod text {
        use super::*;

        #[tokio::test]
        async fn render_error() {
            colored::control::set_override(false);
            let mut string = vec![];

            render_document(
                &mixed_document_output(),
                &RenderOptions::default(),
                &mut string,
            )
            .await
            .unwrap();

            assert_snapshot!(str::from_utf8(&string).unwrap());
        }

        #[tokio::test]
        async fn render_success_and_error_with_verbose_option() {
            colored::control::set_override(false);
            let mut string = vec![];

            render_document(
                &mixed_document_output(),
                &RenderOptions::default().set_verbose(true),
                &mut string,
            )
            .await
            .unwrap();

            assert_snapshot!(str::from_utf8(&string).unwrap());
        }

        #[tokio::test]
        async fn render_success() {
            colored::control::set_override(false);
            let mut string = vec![];

            render_document(
                &successful_document_output(),
                &RenderOptions::default(),
                &mut string,
            )
            .await
            .unwrap();

            assert_snapshot!(str::from_utf8(&string).unwrap());
        }

        #[tokio::test]
        async fn render_success_with_verbose_option() {
            colored::control::set_override(false);
            let mut string = vec![];

            render_document(
                &successful_document_output(),
                &RenderOptions::default().set_verbose(true),
                &mut string,
            )
            .await
            .unwrap();

            assert_snapshot!(str::from_utf8(&string).unwrap());
        }

        #[tokio::test]
        async fn render_data_urls_without_bodies() {
            colored::control::set_override(false);
            let mut string = vec![];

            render_document(
                &data_document_output(),
                &RenderOptions::default().set_verbose(true),
                &mut string,
            )
            .await
            .unwrap();

            assert_snapshot!(str::from_utf8(&string).unwrap());
        }
    }
}
