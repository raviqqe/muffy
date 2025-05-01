// TODO Allow `robots.txt` files as documents.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DocumentType {
    Html,
    Sitemap,
}
