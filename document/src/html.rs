//! HTML documents.

#![allow(missing_docs)]

mod document;
mod element;
mod node;
mod parse;

pub use self::{document::*, element::*, node::*, parse::*};
