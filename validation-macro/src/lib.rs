//! Macros for document validation.

use proc_macro::TokenStream;
use quote::quote;

/// Generates HTML validation functions.
#[proc_macro]
pub fn html(_input: TokenStream) -> TokenStream {
    // TODO Generate validation functions of `validate_element` for HTML against the
    // `Element` type in the `muffy-document` crate based on the HTML
    // specification in the Relax NG Compact syntax at the `schema` directory
    // using the parser in the `muffy-rnc` crate.

    quote! {}.into()
}
