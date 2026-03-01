mod node;

pub use self::node::Node;
use html5ever::{parse_document, tendril::TendrilSink};
use markup5ever_rcdom::RcDom;
use node::Document;
use std::io;

/// Parses an HTML document.
pub fn parse(source: &str) -> Result<Document, io::Error> {
    parse_document(RcDom::default(), Default::default())
        .from_utf8()
        .read_from(source)
        .map(|dom| Document::from_markup5ever(&dom.document).into())
}
