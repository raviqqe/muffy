use alloc::borrow::Cow;
use data_url::DataUrl;

pub fn abbreviate_url(url: &str) -> Cow<'_, str> {
    DataUrl::process(url).map_or(Cow::Borrowed(url), |data_url| {
        format!("data:{}", data_url.mime_type()).into()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn keep_http_url() {
        assert_eq!(abbreviate_url("https://foo.com/"), "https://foo.com/");
    }

    #[test]
    fn abbreviate_data_url() {
        assert_eq!(
            abbreviate_url("data:image/svg+xml,<svg/>"),
            "data:image/svg+xml"
        );
    }

    #[test]
    fn abbreviate_base64_data_url() {
        assert_eq!(
            abbreviate_url("data:image/svg+xml;base64,PHN2Zy8+"),
            "data:image/svg+xml"
        );
    }

    #[test]
    fn abbreviate_data_url_with_media_type_parameter() {
        assert_eq!(
            abbreviate_url("data:image/svg+xml;charset=utf-8,<svg/>"),
            "data:image/svg+xml;charset=utf-8"
        );
    }

    #[test]
    fn abbreviate_data_url_with_uppercase_media_type() {
        assert_eq!(
            abbreviate_url("data:IMAGE/SVG+XML,<svg/>"),
            "data:image/svg+xml"
        );
    }

    #[test]
    fn abbreviate_data_url_with_default_media_type() {
        assert_eq!(
            abbreviate_url("data:,foo"),
            "data:text/plain;charset=US-ASCII"
        );
    }

    #[test]
    fn abbreviate_data_url_with_fragment() {
        assert_eq!(
            abbreviate_url("data:image/svg+xml,<svg/>#icon"),
            "data:image/svg+xml"
        );
    }

    #[test]
    fn keep_data_url_without_comma() {
        assert_eq!(abbreviate_url("data:image/svg+xml"), "data:image/svg+xml");
    }
}
