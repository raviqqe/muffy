//! The [Relax NG Compact](https://relaxng.org/compact.html#annotations) syntax.

mod ast;
mod parse;

pub use self::{ast::*, parse::*};
