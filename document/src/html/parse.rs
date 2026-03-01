use super::document::Document;
pub use super::node::Node;
use html5ever::{parse_document, tendril::TendrilSink};
use markup5ever_rcdom::RcDom;
use std::io;

/// Parses an HTML document.
pub fn parse(source: &str) -> Result<Document, io::Error> {
    let mut source = source.as_bytes();

    parse_document(RcDom::default(), Default::default())
        .from_utf8()
        .read_from(&mut source)
        .map(|dom| Document::from_markup5ever(&dom.document))
}
