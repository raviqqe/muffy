use alloc::borrow::Cow;

const DATA_SCHEME_PREFIX: &str = "data:";
const MAX_DATA_URL_LENGTH: usize = 64;

pub fn truncate_url(url: &str) -> Cow<'_, str> {
    if url
        .get(..DATA_SCHEME_PREFIX.len())
        .is_some_and(|scheme| scheme.eq_ignore_ascii_case(DATA_SCHEME_PREFIX))
        && url.chars().count() > MAX_DATA_URL_LENGTH
    {
        url.chars()
            .take(MAX_DATA_URL_LENGTH)
            .chain("...".chars())
            .collect::<String>()
            .into()
    } else {
        url.into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn keep_http_url() {
        assert_eq!(truncate_url("https://foo.com/"), "https://foo.com/");
    }

    #[test]
    fn keep_long_http_url() {
        let url = format!("https://foo.com/{}", "a".repeat(100));

        assert_eq!(truncate_url(&url), url);
    }

    #[test]
    fn keep_short_data_url() {
        assert_eq!(
            truncate_url("data:image/svg+xml,<svg/>"),
            "data:image/svg+xml,<svg/>"
        );
    }

    #[test]
    fn keep_data_url_of_maximum_length() {
        let url = format!("data:,{}", "a".repeat(58));

        assert_eq!(truncate_url(&url), url);
    }

    #[test]
    fn truncate_long_data_url() {
        assert_eq!(
            truncate_url(&format!("data:,{}", "a".repeat(59))),
            format!("data:,{}...", "a".repeat(58))
        );
    }

    #[test]
    fn truncate_base64_data_url() {
        assert_eq!(
            truncate_url(
                "data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciLz4="
            ),
            "data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcm..."
        );
    }

    #[test]
    fn truncate_data_url_without_comma() {
        assert_eq!(
            truncate_url(&format!("data:{}", "a".repeat(100))),
            format!("data:{}...", "a".repeat(59))
        );
    }

    #[test]
    fn truncate_data_url_with_uppercase_scheme() {
        assert_eq!(
            truncate_url(&format!("DATA:,{}", "a".repeat(100))),
            format!("DATA:,{}...", "a".repeat(58))
        );
    }

    #[test]
    fn truncate_multi_byte_data_url() {
        assert_eq!(
            truncate_url(&format!("data:,{}", "あ".repeat(70))),
            format!("data:,{}...", "あ".repeat(58))
        );
    }
}
