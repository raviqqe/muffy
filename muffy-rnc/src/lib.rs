//! The [Relax NG Compact](https://relaxng.org/compact.html#annotations) syntax.

extern crate alloc;

mod ast;
mod parse;
mod schema;

pub use self::{ast::*, parse::*, schema::*};
